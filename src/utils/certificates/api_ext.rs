mod private_keys_create_params;
mod private_keys_export_params;
mod private_keys_update_params;
mod templates_create_params;
mod templates_generate_params;
mod templates_update_params;

pub use self::{
    private_keys_create_params::PrivateKeysCreateParams,
    private_keys_export_params::PrivateKeysExportParams,
    private_keys_update_params::PrivateKeysUpdateParams,
    templates_create_params::TemplatesCreateParams,
    templates_generate_params::TemplatesGenerateParams,
    templates_update_params::TemplatesUpdateParams,
};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{SharedResource, UserId, UserShare},
    utils::{
        certificates::{
            CertificateTemplate, ExportFormat, ExtendedKeyUsage, KeyUsage, PrivateKey,
            PrivateKeyAlgorithm, SignatureAlgorithm,
        },
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
    },
};
use anyhow::{anyhow, bail};
use openssl::{
    asn1::Asn1Time,
    bn::{BigNum, MsbOption},
    dsa::Dsa,
    ec::{EcGroup, EcKey},
    error::ErrorStack,
    hash::MessageDigest,
    nid::Nid,
    pkcs12::Pkcs12,
    pkey::{PKey, Private},
    rsa::Rsa,
    symm::Cipher,
    x509::{X509, X509Builder, X509NameBuilder, extension},
};
use std::{
    io::{Cursor, Write},
    time::Instant,
};
use time::OffsetDateTime;
use tracing::debug;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// API extension to work with certificates utilities.
pub struct CertificatesApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> CertificatesApiExt<'a, DR, ET> {
    /// Creates Certificates API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves the private key with the specified ID.
    pub async fn get_private_key(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<PrivateKey>> {
        self.api
            .db
            .certificates()
            .get_private_key(user_id, id)
            .await
    }

    /// Generate private key with the specified parameters and stores it in the database.
    pub async fn create_private_key(
        &self,
        user_id: UserId,
        params: PrivateKeysCreateParams,
    ) -> anyhow::Result<PrivateKey> {
        Self::assert_private_key_name(&params.key_name)?;

        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let private_key = PrivateKey {
            id: Uuid::now_v7(),
            name: params.key_name,
            alg: params.alg,
            pkcs8: Self::export_private_key_to_pkcs8(
                Self::generate_private_key(params.alg)?,
                params.passphrase.as_deref(),
            )?,
            encrypted: params.passphrase.is_some(),
            created_at,
            updated_at: created_at,
        };

        self.api
            .db
            .certificates()
            .insert_private_key(user_id, &private_key)
            .await?;

        Ok(private_key)
    }

    /// Updates private key (only name and passphrases are updatable).
    pub async fn update_private_key(
        &self,
        user_id: UserId,
        id: Uuid,
        params: PrivateKeysUpdateParams,
    ) -> anyhow::Result<()> {
        let includes_new_passphrase =
            params.passphrase.is_some() || params.new_passphrase.is_some();
        if params.key_name.is_none() && !includes_new_passphrase {
            bail!(SecutilsError::client(format!(
                "Either new name or passphrase should be provided ({id})."
            )));
        }

        if includes_new_passphrase && params.passphrase == params.new_passphrase {
            bail!(SecutilsError::client(format!(
                "New private key passphrase should be different from the current passphrase ({id})."
            )));
        }

        let Some(private_key) = self.get_private_key(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{id}') is not found."
            )));
        };

        // If name update is needed, extract it from parameters.
        let name = if let Some(name) = params.key_name {
            Self::assert_private_key_name(&name)?;
            name.to_string()
        } else {
            private_key.name
        };

        // If passphrase update is needed, try to decrypt private key using the provided passphrase.
        let (pkcs8, encrypted) = if params.passphrase != params.new_passphrase {
            let pkcs8_private_key = Self::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                params.passphrase.as_deref(),
            )
            .map_err(|err| {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Unable to decrypt private key ('{id}') with the provided passphrase."
                )))
            })?;
            (
                Self::export_private_key_to_pkcs8(
                    pkcs8_private_key,
                    params.new_passphrase.as_deref(),
                )?,
                params.new_passphrase.is_some(),
            )
        } else {
            (private_key.pkcs8, private_key.encrypted)
        };

        // Preserve timestamp only up to seconds.
        let updated_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        self.api
            .db
            .certificates()
            .update_private_key(
                user_id,
                &PrivateKey {
                    name,
                    pkcs8,
                    encrypted,
                    updated_at,
                    ..private_key
                },
            )
            .await
    }

    /// Removes private key with the specified ID.
    pub async fn remove_private_key(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .certificates()
            .remove_private_key(user_id, id)
            .await
    }

    /// Exports private key with the specified ID to the specified format and passphrase.
    pub async fn export_private_key(
        &self,
        user_id: UserId,
        id: Uuid,
        params: PrivateKeysExportParams,
    ) -> anyhow::Result<Vec<u8>> {
        let Some(private_key) = self.get_private_key(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{id}') is not found."
            )));
        };

        // Try to decrypt private key using the provided passphrase.
        let pkcs8_private_key =
            Self::import_private_key_from_pkcs8(&private_key.pkcs8, params.passphrase.as_deref())
                .map_err(|err| {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Unable to decrypt private key ('{id}') with the provided passphrase."
                )))
            })?;

        let export_result = match params.format {
            ExportFormat::Pem => Self::export_private_key_to_pem(
                pkcs8_private_key,
                params.export_passphrase.as_deref(),
            ),
            ExportFormat::Pkcs8 => Self::export_private_key_to_pkcs8(
                pkcs8_private_key,
                params.export_passphrase.as_deref(),
            ),
            ExportFormat::Pkcs12 => Self::export_private_key_to_pkcs12(
                &private_key.name,
                &pkcs8_private_key,
                params.export_passphrase.as_deref(),
            ),
        };

        export_result.map_err(|err| {
            SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                "Unable to export private key ('{id}') to the specified format ('{:?}').",
                params.format
            )))
            .into()
        })
    }

    /// Retrieves all private keys that belong to the specified user.
    pub async fn get_private_keys(&self, user_id: UserId) -> anyhow::Result<Vec<PrivateKey>> {
        self.api.db.certificates().get_private_keys(user_id).await
    }

    /// Retrieves the certificate template with the specified ID.
    pub async fn get_certificate_template(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<CertificateTemplate>> {
        self.api
            .db
            .certificates()
            .get_certificate_template(user_id, id)
            .await
    }

    /// Creates certificate template with the specified parameters and stores it in the database.
    pub async fn create_certificate_template(
        &self,
        user_id: UserId,
        params: TemplatesCreateParams,
    ) -> anyhow::Result<CertificateTemplate> {
        Self::assert_certificate_template_name(&params.template_name)?;

        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let certificate_template = CertificateTemplate {
            id: Uuid::now_v7(),
            name: params.template_name,
            attributes: params.attributes,
            created_at,
            updated_at: created_at,
        };

        self.api
            .db
            .certificates()
            .insert_certificate_template(user_id, &certificate_template)
            .await?;

        Ok(certificate_template)
    }

    /// Updates certificate template.
    pub async fn update_certificate_template(
        &self,
        user_id: UserId,
        id: Uuid,
        params: TemplatesUpdateParams,
    ) -> anyhow::Result<()> {
        if let Some(name) = &params.template_name {
            Self::assert_certificate_template_name(name)?;
        } else if params.attributes.is_none() {
            bail!(SecutilsError::client(format!(
                "Either new name or attributes should be provided ({id})."
            )));
        }

        let Some(certificate_template) = self.get_certificate_template(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Certificate template ('{id}') is not found."
            )));
        };

        // Preserve timestamp only up to seconds.
        let updated_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        self.api
            .db
            .certificates()
            .update_certificate_template(
                user_id,
                &CertificateTemplate {
                    name: if let Some(name) = params.template_name {
                        name
                    } else {
                        certificate_template.name
                    },
                    attributes: if let Some(attributes) = params.attributes {
                        attributes
                    } else {
                        certificate_template.attributes
                    },
                    updated_at,
                    ..certificate_template
                },
            )
            .await
    }

    /// Removes certificate template with the specified ID.
    pub async fn remove_certificate_template(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<()> {
        self.unshare_certificate_template(user_id, id).await?;
        self.api
            .db
            .certificates()
            .remove_certificate_template(user_id, id)
            .await
    }

    /// Generates private key and certificate pair from the certificate template.
    pub async fn generate_self_signed_certificate(
        &self,
        user_id: UserId,
        template_id: Uuid,
        params: TemplatesGenerateParams,
    ) -> anyhow::Result<Vec<u8>> {
        let Some(certificate_template) =
            self.get_certificate_template(user_id, template_id).await?
        else {
            bail!(SecutilsError::client(format!(
                "Certificate template ('{template_id}') is not found."
            )));
        };

        // Create X509 certificate builder pre-filled with the specified template properties.
        let mut certificate_builder = Self::create_x509_certificate_builder(&certificate_template)?;

        // Generate private key, set certificate public key and sign it.
        let private_key =
            Self::generate_private_key(certificate_template.attributes.key_algorithm)?;
        certificate_builder.set_pubkey(&private_key)?;
        certificate_builder.sign(
            &private_key,
            Self::get_message_digest(
                certificate_template.attributes.key_algorithm,
                certificate_template.attributes.signature_algorithm,
            )?,
        )?;

        let certificate = certificate_builder.build();
        Ok(match params.format {
            ExportFormat::Pem => Self::export_key_pair_to_pem_archive(
                certificate,
                private_key,
                params.passphrase.as_deref(),
            )?,
            ExportFormat::Pkcs8 => {
                Self::export_private_key_to_pkcs8(private_key, params.passphrase.as_deref())?
            }
            ExportFormat::Pkcs12 => Self::export_key_pair_to_pkcs12(
                &certificate_template.name,
                &private_key,
                &certificate,
                params.passphrase.as_deref(),
            )?,
        })
    }

    /// Retrieves all certificate templates that belong to the specified user.
    pub async fn get_certificate_templates(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<CertificateTemplate>> {
        self.api
            .db
            .certificates()
            .get_certificate_templates(user_id)
            .await
    }

    /// Shares certificate template with the specified ID.
    pub async fn share_certificate_template(
        &self,
        user_id: UserId,
        template_id: Uuid,
    ) -> anyhow::Result<UserShare> {
        let users_api = self.api.users();
        let template_resource = SharedResource::CertificateTemplate { template_id };

        // Return early if policy is already shared.
        if let Some(user_share) = users_api
            .get_user_share_by_resource(user_id, &template_resource)
            .await?
        {
            return Ok(user_share);
        }

        // Ensure that certificate template exists.
        if self
            .get_certificate_template(user_id, template_id)
            .await?
            .is_none()
        {
            bail!(SecutilsError::client(format!(
                "Certificate template ('{template_id}') is not found."
            )));
        }

        // Create new user share.
        let user_share = UserShare {
            id: Default::default(),
            user_id,
            resource: template_resource,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };
        users_api
            .insert_user_share(&user_share)
            .await
            .map(|_| user_share)
    }

    /// Unshares certificate template with the specified ID.
    pub async fn unshare_certificate_template(
        &self,
        user_id: UserId,
        template_id: Uuid,
    ) -> anyhow::Result<Option<UserShare>> {
        let users_api = self.api.users();

        // Check if template is shared.
        let Some(user_share) = users_api
            .get_user_share_by_resource(
                user_id,
                &SharedResource::CertificateTemplate { template_id },
            )
            .await?
        else {
            return Ok(None);
        };

        users_api.remove_user_share(user_share.id).await
    }

    /// Generates private key with the specified parameters.
    fn generate_private_key(alg: PrivateKeyAlgorithm) -> anyhow::Result<PKey<Private>> {
        let execute_start = Instant::now();
        let private_key = match alg {
            PrivateKeyAlgorithm::Rsa { key_size } => {
                PKey::from_rsa(Rsa::generate(key_size as u32)?)?
            }
            PrivateKeyAlgorithm::Dsa { key_size } => {
                PKey::from_dsa(Dsa::generate(key_size as u32)?)?
            }
            PrivateKeyAlgorithm::Ecdsa { curve } => {
                let ec_group = EcGroup::from_curve_name(Nid::from_raw(curve as i32))?;
                PKey::from_ec_key(EcKey::generate(&ec_group)?)?
            }
            PrivateKeyAlgorithm::Ed25519 => PKey::generate_ed25519()?,
        };

        debug!(
            "Generated a private key with {alg:?} parameters ({} elapsed).",
            humantime::format_duration(execute_start.elapsed())
        );

        Ok(private_key)
    }

    fn get_message_digest(
        pk_alg: PrivateKeyAlgorithm,
        sig_alg: SignatureAlgorithm,
    ) -> anyhow::Result<MessageDigest> {
        match (pk_alg, sig_alg) {
            (PrivateKeyAlgorithm::Rsa { .. }, SignatureAlgorithm::Md5) => Ok(MessageDigest::md5()),
            (
                PrivateKeyAlgorithm::Rsa { .. }
                | PrivateKeyAlgorithm::Dsa { .. }
                | PrivateKeyAlgorithm::Ecdsa { .. },
                SignatureAlgorithm::Sha1,
            ) => Ok(MessageDigest::sha1()),
            (
                PrivateKeyAlgorithm::Rsa { .. }
                | PrivateKeyAlgorithm::Dsa { .. }
                | PrivateKeyAlgorithm::Ecdsa { .. },
                SignatureAlgorithm::Sha256,
            ) => Ok(MessageDigest::sha256()),
            (
                PrivateKeyAlgorithm::Rsa { .. } | PrivateKeyAlgorithm::Ecdsa { .. },
                SignatureAlgorithm::Sha384,
            ) => Ok(MessageDigest::sha384()),
            (
                PrivateKeyAlgorithm::Rsa { .. } | PrivateKeyAlgorithm::Ecdsa { .. },
                SignatureAlgorithm::Sha512,
            ) => Ok(MessageDigest::sha512()),
            (PrivateKeyAlgorithm::Ed25519, SignatureAlgorithm::Ed25519) => {
                Ok(MessageDigest::null())
            }
            _ => Err(anyhow!(
                "Public key ({:?}) and signature ({:?}) algorithms are not compatible",
                pk_alg,
                sig_alg
            )),
        }
    }

    fn export_private_key_to_pem(
        private_key: PKey<Private>,
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>, ErrorStack> {
        match passphrase {
            None => private_key.private_key_to_pem_pkcs8(),
            Some(passphrase) => private_key
                .private_key_to_pem_pkcs8_passphrase(Cipher::aes_256_cbc(), passphrase.as_bytes()),
        }
    }

    fn export_key_pair_to_pem_archive(
        certificate: X509,
        private_key: PKey<Private>,
        passphrase: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        // 64kb should be more than enough for the certificate + private key.
        let mut zip_buffer = [0; 65536];
        let size = {
            let mut zip = ZipWriter::new(Cursor::new(&mut zip_buffer[..]));

            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("certificate.crt", options)?;
            zip.write_all(&certificate.to_pem()?)?;

            zip.start_file("private_key.key", options)?;
            zip.write_all(&Self::export_private_key_to_pkcs8(private_key, passphrase)?)?;

            zip.finish()?.position() as usize
        };

        Ok(zip_buffer[..size].to_vec())
    }

    fn export_private_key_to_pkcs8(
        private_key: PKey<Private>,
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>, ErrorStack> {
        if let Some(passphrase) = passphrase {
            // AEAD ciphers not supported in this command.
            private_key
                .private_key_to_pkcs8_passphrase(Cipher::aes_256_cbc(), passphrase.as_bytes())
        } else {
            private_key.private_key_to_pkcs8()
        }
    }

    fn export_private_key_to_pkcs12(
        name: &str,
        private_key: &PKey<Private>,
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>, ErrorStack> {
        Pkcs12::builder()
            .name(name)
            .pkey(private_key)
            .build2(passphrase.unwrap_or_default())?
            .to_der()
    }

    fn export_key_pair_to_pkcs12(
        name: &str,
        private_key: &PKey<Private>,
        certificate: &X509,
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>, ErrorStack> {
        Pkcs12::builder()
            .name(name)
            .pkey(private_key)
            .cert(certificate)
            .build2(passphrase.unwrap_or_default())?
            .to_der()
    }

    fn import_private_key_from_pkcs8(
        pkcs8: &[u8],
        passphrase: Option<&str>,
    ) -> Result<PKey<Private>, ErrorStack> {
        if let Some(passphrase) = passphrase {
            PKey::private_key_from_pkcs8_passphrase(pkcs8, passphrase.as_bytes())
        } else {
            PKey::private_key_from_pkcs8(pkcs8)
        }
    }

    fn create_x509_certificate_builder(
        certificate_template: &CertificateTemplate,
    ) -> anyhow::Result<X509Builder> {
        let mut x509_name = X509NameBuilder::new()?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "CN",
            &certificate_template.attributes.common_name,
        )?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "C",
            &certificate_template.attributes.country,
        )?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "ST",
            &certificate_template.attributes.state_or_province,
        )?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "L",
            &certificate_template.attributes.locality,
        )?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "O",
            &certificate_template.attributes.organization,
        )?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "OU",
            &certificate_template.attributes.organizational_unit,
        )?;
        let x509_name = x509_name.build();

        let mut x509 = X509::builder()?;
        x509.set_subject_name(&x509_name)?;
        x509.set_issuer_name(&x509_name)?;
        x509.set_version(certificate_template.attributes.version.value())?;

        let mut basic_constraint = extension::BasicConstraints::new();
        if certificate_template.attributes.is_ca {
            basic_constraint.ca();
        }
        x509.append_extension(basic_constraint.critical().build()?)?;

        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        x509.set_serial_number(&serial_number)?;

        let not_before = Asn1Time::from_unix(
            certificate_template
                .attributes
                .not_valid_before
                .unix_timestamp(),
        )?;
        x509.set_not_before(&not_before)?;
        let not_after = Asn1Time::from_unix(
            certificate_template
                .attributes
                .not_valid_after
                .unix_timestamp(),
        )?;
        x509.set_not_after(&not_after)?;

        if let Some(ref key_usage) = certificate_template.attributes.key_usage {
            let mut key_usage_ext = extension::KeyUsage::new();

            for key_usage in key_usage {
                match key_usage {
                    KeyUsage::DigitalSignature => key_usage_ext.digital_signature(),
                    KeyUsage::NonRepudiation => key_usage_ext.non_repudiation(),
                    KeyUsage::KeyEncipherment => key_usage_ext.key_encipherment(),
                    KeyUsage::DataEncipherment => key_usage_ext.data_encipherment(),
                    KeyUsage::KeyAgreement => key_usage_ext.key_agreement(),
                    KeyUsage::KeyCertificateSigning => key_usage_ext.key_cert_sign(),
                    KeyUsage::CrlSigning => key_usage_ext.crl_sign(),
                    KeyUsage::EncipherOnly => key_usage_ext.encipher_only(),
                    KeyUsage::DecipherOnly => key_usage_ext.decipher_only(),
                };
            }

            x509.append_extension(key_usage_ext.critical().build()?)?;
        }

        if let Some(ref key_usage) = certificate_template.attributes.extended_key_usage {
            let mut key_usage_ext = extension::ExtendedKeyUsage::new();

            for key_usage in key_usage {
                match key_usage {
                    ExtendedKeyUsage::TlsWebServerAuthentication => key_usage_ext.server_auth(),
                    ExtendedKeyUsage::TlsWebClientAuthentication => key_usage_ext.client_auth(),
                    ExtendedKeyUsage::CodeSigning => key_usage_ext.code_signing(),
                    ExtendedKeyUsage::EmailProtection => key_usage_ext.email_protection(),
                    ExtendedKeyUsage::TimeStamping => key_usage_ext.time_stamping(),
                };
            }

            x509.append_extension(key_usage_ext.critical().build()?)?;
        }

        let subject_key_identifier =
            extension::SubjectKeyIdentifier::new().build(&x509.x509v3_context(None, None))?;
        x509.append_extension(subject_key_identifier)?;

        Ok(x509)
    }

    fn set_x509_name_attribute(
        x509_name: &mut X509NameBuilder,
        attribute_key: &str,
        attribute_value: &Option<String>,
    ) -> anyhow::Result<()> {
        if attribute_key.is_empty() {
            return Ok(());
        }

        if let Some(attribute_value) = attribute_value {
            if !attribute_value.is_empty() {
                x509_name.append_entry_by_text(attribute_key, attribute_value)?;
            }
        }

        Ok(())
    }

    fn assert_private_key_name(name: &str) -> Result<(), SecutilsError> {
        if name.is_empty() {
            return Err(SecutilsError::client("Private key name cannot be empty."));
        }

        if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            return Err(SecutilsError::client(format!(
                "Private key name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        Ok(())
    }

    fn assert_certificate_template_name(name: &str) -> Result<(), SecutilsError> {
        if name.is_empty() {
            return Err(SecutilsError::client(
                "Certificate template name cannot be empty.",
            ));
        }

        if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            return Err(SecutilsError::client(format!(
                "Certificate template name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        Ok(())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with certificates utility.
    pub fn certificates(&self) -> CertificatesApiExt<DR, ET> {
        CertificatesApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{CertificatesApiExt, PrivateKeysCreateParams};
    use crate::{
        tests::{MockResolver, mock_api, mock_user},
        utils::certificates::{
            CertificateAttributes, ExportFormat, ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm,
            PrivateKeyEllipticCurve, PrivateKeySize, SignatureAlgorithm, Version,
            api_ext::{
                PrivateKeysExportParams, PrivateKeysUpdateParams, TemplatesCreateParams,
                TemplatesGenerateParams, TemplatesUpdateParams,
            },
        },
    };
    use insta::assert_debug_snapshot;
    use lettre::transport::stub::AsyncStubTransport;
    use openssl::{hash::MessageDigest, pkcs12::Pkcs12};
    use sqlx::PgPool;
    use time::OffsetDateTime;

    fn get_mock_certificate_attributes() -> anyhow::Result<CertificateAttributes> {
        Ok(CertificateAttributes {
            common_name: Some("my-common-name".to_string()),
            country: Some("DE".to_string()),
            state_or_province: Some("BE".to_string()),
            locality: None,
            organization: None,
            organizational_unit: None,
            key_algorithm: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024,
            },
            signature_algorithm: SignatureAlgorithm::Sha256,
            not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
            not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
            version: Version::One,
            is_ca: true,
            key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
            extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
        })
    }

    #[sqlx::test]
    async fn can_create_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        for pass in [Some("pass"), Some(""), None] {
            for (alg, bits) in [
                (
                    PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    1024,
                ),
                (
                    PrivateKeyAlgorithm::Dsa {
                        key_size: PrivateKeySize::Size2048,
                    },
                    2048,
                ),
                (
                    PrivateKeyAlgorithm::Ecdsa {
                        curve: PrivateKeyEllipticCurve::SECP521R1,
                    },
                    521,
                ),
                (PrivateKeyAlgorithm::Ed25519, 256),
            ] {
                let private_key = certificates
                    .create_private_key(
                        mock_user.id,
                        PrivateKeysCreateParams {
                            key_name: format!("pk-{:?}-{:?}", alg, pass),
                            alg,
                            passphrase: pass.map(|p| p.to_string()),
                        },
                    )
                    .await?;
                assert_eq!(private_key.alg, alg);

                let imported_key =
                    CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                        &private_key.pkcs8,
                        pass,
                    )?;
                assert_eq!(imported_key.bits(), bits);
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn fails_to_create_private_key_if_name_is_invalid(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        assert_debug_snapshot!(
            certificates
                .create_private_key(
                    mock_user.id,
                    PrivateKeysCreateParams {
                        key_name: "".to_string(),
                        alg: PrivateKeyAlgorithm::Ed25519,
                        passphrase: None,
                    },
                )
                .await,
            @r###"
        Err(
            "Private key name cannot be empty.",
        )
        "###
        );

        assert_debug_snapshot!(
            certificates
                .create_private_key(
                    mock_user.id,
                    PrivateKeysCreateParams {
                        key_name: "a".repeat(101),
                        alg: PrivateKeyAlgorithm::Ed25519,
                        passphrase: None,
                    },
                )
                .await,
            @r###"
        Err(
            "Private key name cannot be longer than 100 characters.",
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_change_private_key_passphrase(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        // Decrypting without password should succeed.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_ok()
        );

        // Set passphrase.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: None,
                    passphrase: None,
                    new_passphrase: Some("pass".to_string()),
                },
            )
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_err()
        );
        // Decrypting with passphrase should succeed.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        // Change passphrase.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: None,
                    passphrase: Some("pass".to_string()),
                    new_passphrase: Some("pass-1".to_string()),
                },
            )
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_err()
        );

        // Decrypting with old passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should succeed.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass-1"),
            )
            .is_ok()
        );

        // Remove passphrase.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: None,
                    passphrase: Some("pass-1".to_string()),
                    new_passphrase: None,
                },
            )
            .await?;

        // Decrypting without passphrase should succeed.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_ok()
        );

        // Decrypting with old passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should fail.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass-1"),
            )
            .is_err()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_change_private_key_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: Some("pass".to_string()),
                },
            )
            .await?;

        // Update name.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: Some("pk-new".to_string()),
                    passphrase: None,
                    new_passphrase: None,
                },
            )
            .await?;

        // Name should change, and pkcs8 shouldn't change.
        let updated_private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(updated_private_key.name, "pk-new");
        assert_eq!(private_key.pkcs8, updated_private_key.pkcs8);
        assert_eq!(private_key.encrypted, updated_private_key.encrypted);

        // Decrypting with the old passphrase should succeed.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        // Change both name and passphrase.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: Some("pk-new-new".to_string()),
                    passphrase: Some("pass".to_string()),
                    new_passphrase: Some("pass-1".to_string()),
                },
            )
            .await?;

        // Name should change and decrypting with old passphrase should fail.
        let updated_private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(updated_private_key.name, "pk-new-new");
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &updated_private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should succeed.
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &updated_private_key.pkcs8,
                Some("pass-1"),
            )
            .is_ok()
        );

        // Remove passphrase and return old name back.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: Some("pk".to_string()),
                    passphrase: Some("pass-1".to_string()),
                    new_passphrase: None,
                },
            )
            .await?;

        // Name should change and decrypting without passphrase should succeed.
        let updated_private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(updated_private_key.name, "pk");
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &updated_private_key.pkcs8,
                None,
            )
            .is_ok()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fails_to_update_private_key_if_params_are_invalid(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;
        // Invalid name.
        assert_debug_snapshot!(certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: Some("".to_string()),
                    passphrase: None,
                    new_passphrase: None,
                },
            )
            .await,
            @r###"
        Err(
            "Private key name cannot be empty.",
        )
        "###
        );

        // Invalid name.
        assert_debug_snapshot!(certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: Some("a".repeat(101)),
                    passphrase: None,
                    new_passphrase: None,
                },
            )
            .await,
            @r###"
        Err(
            "Private key name cannot be longer than 100 characters.",
        )
        "###
        );

        // Invalid params.
        assert_eq!(
            certificates
                .update_private_key(
                    mock_user.id,
                    private_key.id,
                    PrivateKeysUpdateParams {
                        key_name: None,
                        passphrase: None,
                        new_passphrase: None,
                    },
                )
                .await
                .unwrap_err()
                .to_string(),
            format!(
                "Either new name or passphrase should be provided ({}).",
                private_key.id
            )
        );

        // Invalid passphrases.
        assert_eq!(
            certificates
                .update_private_key(
                    mock_user.id,
                    private_key.id,
                    PrivateKeysUpdateParams {
                        key_name: None,
                        passphrase: Some("some".to_string()),
                        new_passphrase: Some("some".to_string()),
                    },
                )
                .await
                .unwrap_err()
                .to_string(),
            format!(
                "New private key passphrase should be different from the current passphrase ({}).",
                private_key.id
            )
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_export_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create private key without passphrase.
        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysExportParams {
                    format: ExportFormat::Pkcs8,
                    passphrase: None,
                    export_passphrase: None,
                },
            )
            .await?;
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8, None,
            )
            .is_ok()
        );
        // Export private key with passphrase and make sure it can be imported with passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysExportParams {
                    format: ExportFormat::Pkcs8,
                    passphrase: None,
                    export_passphrase: Some("pass".to_string()),
                },
            )
            .await?;
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        // Set passphrase and repeat.
        certificates
            .update_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysUpdateParams {
                    key_name: None,
                    passphrase: None,
                    new_passphrase: Some("pass".to_string()),
                },
            )
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysExportParams {
                    format: ExportFormat::Pkcs8,
                    passphrase: Some("pass".to_string()),
                    export_passphrase: None,
                },
            )
            .await?;
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8, None,
            )
            .is_ok()
        );
        // Export private key with passphrase and make sure it can be imported with passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                PrivateKeysExportParams {
                    format: ExportFormat::Pkcs8,
                    passphrase: Some("pass".to_string()),
                    export_passphrase: Some("pass".to_string()),
                },
            )
            .await?;
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_private_key(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;
        assert_eq!(
            private_key,
            certificates
                .get_private_key(mock_user.id, private_key.id)
                .await?
                .unwrap()
        );

        certificates
            .remove_private_key(mock_user.id, private_key.id)
            .await?;

        assert!(
            certificates
                .get_private_key(mock_user.id, private_key.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_return_multiple_private_keys(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        assert!(
            certificates
                .get_private_keys(mock_user.id)
                .await?
                .is_empty()
        );

        let private_key_one = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await
            .map(|mut private_key| {
                private_key.pkcs8.clear();
                private_key
            })?;
        assert_eq!(
            certificates.get_private_keys(mock_user.id).await?,
            vec![private_key_one.clone()]
        );

        let private_key_two = certificates
            .create_private_key(
                mock_user.id,
                PrivateKeysCreateParams {
                    key_name: "pk-2".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await
            .map(|mut private_key| {
                private_key.pkcs8.clear();
                private_key
            })?;
        assert_eq!(
            certificates.get_private_keys(mock_user.id).await?,
            vec![private_key_one.clone(), private_key_two.clone()]
        );

        certificates
            .remove_private_key(mock_user.id, private_key_one.id)
            .await?;
        certificates
            .remove_private_key(mock_user.id, private_key_two.id)
            .await?;

        assert!(
            certificates
                .get_private_keys(mock_user.id)
                .await?
                .is_empty()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_create_certificate_template(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        assert_eq!(certificate_template.name, "ct");
        assert_eq!(
            certificate_template.attributes,
            get_mock_certificate_attributes()?
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fails_to_create_certificate_template_if_name_is_invalid(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        assert_debug_snapshot!(certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await,
            @r###"
        Err(
            "Certificate template name cannot be empty.",
        )
        "###
        );

        assert_debug_snapshot!(certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "a".repeat(101),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await,
            @r###"
        Err(
            "Certificate template name cannot be longer than 100 characters.",
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_change_certificate_template_attributes(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        // Update attributes.
        certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                TemplatesUpdateParams {
                    template_name: None,
                    attributes: Some(CertificateAttributes {
                        common_name: Some("cn-new".to_string()),
                        country: Some("c".to_string()),
                        state_or_province: Some("s".to_string()),
                        locality: None,
                        organization: None,
                        organizational_unit: None,
                        key_algorithm: PrivateKeyAlgorithm::Ed25519,
                        signature_algorithm: SignatureAlgorithm::Md5,
                        not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                        not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                        version: Version::One,
                        is_ca: true,
                        key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                        extended_key_usage: Some(
                            [ExtendedKeyUsage::EmailProtection].into_iter().collect(),
                        ),
                    }),
                },
            )
            .await?;

        // Decrypting without passphrase should fail.
        let certificate_template = certificates
            .get_certificate_template(mock_user.id, certificate_template.id)
            .await?
            .unwrap();
        assert_eq!(certificate_template.name, "ct");
        assert_eq!(
            certificate_template.attributes,
            CertificateAttributes {
                common_name: Some("cn-new".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_change_certificate_template_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        // Update name.
        certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                TemplatesUpdateParams {
                    template_name: Some("ct-new".to_string()),
                    attributes: None,
                },
            )
            .await?;

        // Name should change, and attributes shouldn't change.
        let updated_certificate_template = certificates
            .get_certificate_template(mock_user.id, certificate_template.id)
            .await?
            .unwrap();
        assert_eq!(updated_certificate_template.name, "ct-new");
        assert_eq!(
            certificate_template.attributes,
            get_mock_certificate_attributes()?
        );

        // Change both name and attributes.
        certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                TemplatesUpdateParams {
                    template_name: Some("ct-new-new".to_string()),
                    attributes: Some(CertificateAttributes {
                        common_name: Some("cn-new".to_string()),
                        country: Some("c".to_string()),
                        state_or_province: Some("s".to_string()),
                        locality: None,
                        organization: None,
                        organizational_unit: None,
                        key_algorithm: PrivateKeyAlgorithm::Ed25519,
                        signature_algorithm: SignatureAlgorithm::Md5,
                        not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                        not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                        version: Version::One,
                        is_ca: true,
                        key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                        extended_key_usage: Some(
                            [ExtendedKeyUsage::EmailProtection].into_iter().collect(),
                        ),
                    }),
                },
            )
            .await?;

        let updated_certificate_template = certificates
            .get_certificate_template(mock_user.id, certificate_template.id)
            .await?
            .unwrap();
        assert_eq!(updated_certificate_template.name, "ct-new-new");
        assert_eq!(
            updated_certificate_template.attributes,
            CertificateAttributes {
                common_name: Some("cn-new".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect(),),
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fails_to_update_certificate_template_if_params_are_invalid(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        // Invalid name.
        assert_debug_snapshot!(certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                TemplatesUpdateParams {
                    template_name: Some("".to_string()),
                    attributes: Some(get_mock_certificate_attributes()?),
                },
            )
            .await,
            @r###"
        Err(
            "Certificate template name cannot be empty.",
        )
        "###
        );

        // Invalid name.
        assert_debug_snapshot!(certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                TemplatesUpdateParams {
                    template_name: Some("a".repeat(101)),
                    attributes: Some(get_mock_certificate_attributes()?),
                },
            )
            .await,
            @r###"
        Err(
            "Certificate template name cannot be longer than 100 characters.",
        )
        "###
        );

        // Invalid params.
        assert_eq!(
            certificates
                .update_certificate_template(
                    mock_user.id,
                    certificate_template.id,
                    TemplatesUpdateParams {
                        template_name: None,
                        attributes: None,
                    },
                )
                .await
                .unwrap_err()
                .to_string(),
            format!(
                "Either new name or attributes should be provided ({}).",
                certificate_template.id
            )
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_certificate_template(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        assert_eq!(
            certificate_template,
            certificates
                .get_certificate_template(mock_user.id, certificate_template.id)
                .await?
                .unwrap()
        );

        certificates
            .remove_certificate_template(mock_user.id, certificate_template.id)
            .await?;

        assert!(
            certificates
                .get_certificate_template(mock_user.id, certificate_template.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_return_multiple_certificate_templates(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        assert!(
            certificates
                .get_certificate_templates(mock_user.id)
                .await?
                .is_empty()
        );

        let certificate_template_one = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        assert_eq!(
            certificates.get_certificate_templates(mock_user.id).await?,
            vec![certificate_template_one.clone()]
        );

        let certificate_template_two = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct-2".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        assert_eq!(
            certificates.get_certificate_templates(mock_user.id).await?,
            vec![
                certificate_template_one.clone(),
                certificate_template_two.clone()
            ]
        );

        certificates
            .remove_certificate_template(mock_user.id, certificate_template_one.id)
            .await?;
        certificates
            .remove_certificate_template(mock_user.id, certificate_template_two.id)
            .await?;

        assert!(
            certificates
                .get_certificate_templates(mock_user.id)
                .await?
                .is_empty()
        );

        Ok(())
    }

    #[test]
    fn picks_correct_message_digest() -> anyhow::Result<()> {
        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024,
                },
                SignatureAlgorithm::Md5
            )? == MessageDigest::md5()
        );

        for pk_algorithm in [
            PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024,
            },
            PrivateKeyAlgorithm::Dsa {
                key_size: PrivateKeySize::Size2048,
            },
            PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1,
            },
        ] {
            assert!(
                CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha1
                )? == MessageDigest::sha1()
            );
            assert!(
                CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha256
                )? == MessageDigest::sha256()
            );
        }

        for pk_algorithm in [
            PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024,
            },
            PrivateKeyAlgorithm::Ecdsa {
                curve: PrivateKeyEllipticCurve::SECP256R1,
            },
        ] {
            assert!(
                CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha384
                )? == MessageDigest::sha384()
            );
            assert!(
                CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha512
                )? == MessageDigest::sha512()
            );
        }

        assert!(
            CertificatesApiExt::<MockResolver, AsyncStubTransport>::get_message_digest(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519
            )? == MessageDigest::null()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_generates_x509_certificate(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificate_template = api
            .certificates()
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;

        let exported_certificate_pair = api
            .certificates()
            .generate_self_signed_certificate(
                mock_user.id,
                certificate_template.id,
                TemplatesGenerateParams {
                    format: ExportFormat::Pkcs12,
                    passphrase: None,
                },
            )
            .await?;

        let imported_key_pair = Pkcs12::from_der(&exported_certificate_pair)?.parse2("")?;
        let private_key = imported_key_pair.pkey.unwrap().rsa()?;
        private_key.check_key()?;
        assert_eq!(private_key.size(), 128);

        let certificate = imported_key_pair.cert.unwrap();
        assert_debug_snapshot!(certificate.not_before(), @"Jan  1 10:00:00 2000 GMT");
        assert_debug_snapshot!(certificate.not_after(), @"Jan  1 10:00:00 2010 GMT");

        assert_eq!(
            certificate.public_key()?.public_key_to_der()?,
            private_key.public_key_to_der()?
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_shares_certificate_template(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create and share policy.
        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        let template_share_one = certificates
            .share_certificate_template(mock_user.id, certificate_template.id)
            .await?;

        assert_eq!(
            api.users().get_user_share(template_share_one.id).await?,
            Some(template_share_one.clone())
        );

        // Repetitive sharing should return the same share.
        let template_share_two = certificates
            .share_certificate_template(mock_user.id, certificate_template.id)
            .await?;

        assert_eq!(template_share_one, template_share_two);
        assert_eq!(
            api.users().get_user_share(template_share_one.id).await?,
            Some(template_share_one.clone())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_unshares_certificate_template(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        let template_share_one = certificates
            .share_certificate_template(mock_user.id, certificate_template.id)
            .await?;
        assert_eq!(
            certificates
                .unshare_certificate_template(mock_user.id, certificate_template.id)
                .await?,
            Some(template_share_one.clone())
        );

        assert!(
            api.users()
                .get_user_share(template_share_one.id)
                .await?
                .is_none()
        );

        // Sharing again should return different share.
        let template_share_two = certificates
            .share_certificate_template(mock_user.id, certificate_template.id)
            .await?;
        assert_ne!(template_share_one.id, template_share_two.id);

        assert_eq!(
            certificates
                .unshare_certificate_template(mock_user.id, certificate_template.id)
                .await?,
            Some(template_share_two.clone())
        );

        assert!(
            api.users()
                .get_user_share(template_share_two.id)
                .await?
                .is_none()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_unshares_certificate_template_when_it_is_removed(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create and share template.
        let certificates = api.certificates();
        let certificate_template = certificates
            .create_certificate_template(
                mock_user.id,
                TemplatesCreateParams {
                    template_name: "ct".to_string(),
                    attributes: get_mock_certificate_attributes()?,
                },
            )
            .await?;
        let template_share = certificates
            .share_certificate_template(mock_user.id, certificate_template.id)
            .await?;

        assert_eq!(
            api.users().get_user_share(template_share.id).await?,
            Some(template_share.clone())
        );

        certificates
            .remove_certificate_template(mock_user.id, certificate_template.id)
            .await?;

        assert!(
            api.users()
                .get_user_share(template_share.id)
                .await?
                .is_none()
        );

        Ok(())
    }
}
