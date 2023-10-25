use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{SharedResource, UserId, UserShare},
    utils::{
        CertificateAttributes, CertificateTemplate, ExportFormat, ExtendedKeyUsage, KeyUsage,
        PrivateKey, PrivateKeyAlgorithm, SignatureAlgorithm,
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
    x509::{extension, X509Builder, X509NameBuilder, X509},
};
use std::{
    io::{Cursor, Write},
    time::Instant,
};
use time::OffsetDateTime;
use uuid::Uuid;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

/// API extension to work with certificates utilities.
pub struct CertificatesApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> CertificatesApi<'a, DR, ET> {
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
        name: impl Into<String>,
        alg: PrivateKeyAlgorithm,
        passphrase: Option<&str>,
    ) -> anyhow::Result<PrivateKey> {
        let private_key = PrivateKey {
            id: Uuid::now_v7(),
            name: name.into(),
            alg,
            pkcs8: Self::export_private_key_to_pkcs8(Self::generate_private_key(alg)?, passphrase)?,
            encrypted: passphrase.is_some(),
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
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
        name: Option<&str>,
        passphrase: Option<&str>,
        new_passphrase: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(private_key) = self.get_private_key(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{id}') is not found."
            )));
        };

        // If name update is needed, extract it from parameters.
        let name = if let Some(name) = name {
            name.to_string()
        } else {
            private_key.name
        };

        // If passphrase update is needed, try to decrypt private key using the provided passphrase.
        let (pkcs8, encrypted) = if passphrase != new_passphrase {
            let pkcs8_private_key =
                Self::import_private_key_from_pkcs8(&private_key.pkcs8, passphrase).map_err(
                    |err| {
                        SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                            "Unable to decrypt private key ('{id}') with the provided passphrase."
                        )))
                    },
                )?;
            (
                Self::export_private_key_to_pkcs8(pkcs8_private_key, new_passphrase)?,
                new_passphrase.is_some(),
            )
        } else {
            (private_key.pkcs8, private_key.encrypted)
        };

        self.api
            .db
            .certificates()
            .update_private_key(
                user_id,
                &PrivateKey {
                    name,
                    pkcs8,
                    encrypted,
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
        format: ExportFormat,
        passphrase: Option<&str>,
        export_passphrase: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let Some(private_key) = self.get_private_key(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{id}') is not found."
            )));
        };

        // Try to decrypt private key using the provided passphrase.
        let pkcs8_private_key = Self::import_private_key_from_pkcs8(&private_key.pkcs8, passphrase)
            .map_err(|err| {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Unable to decrypt private key ('{id}') with the provided passphrase."
                )))
            })?;

        let export_result = match format {
            ExportFormat::Pem => {
                Self::export_private_key_to_pem(pkcs8_private_key, export_passphrase)
            }
            ExportFormat::Pkcs8 => {
                Self::export_private_key_to_pkcs8(pkcs8_private_key, export_passphrase)
            }
            ExportFormat::Pkcs12 => Self::export_private_key_to_pkcs12(
                &private_key.name,
                &pkcs8_private_key,
                export_passphrase,
            ),
        };

        export_result.map_err(|err| {
            SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                "Unable to export private key ('{id}') to the specified format ('{format:?}')."
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
        name: impl Into<String>,
        attributes: CertificateAttributes,
    ) -> anyhow::Result<CertificateTemplate> {
        let certificate_template = CertificateTemplate {
            id: Uuid::now_v7(),
            name: name.into(),
            attributes,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
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
        name: Option<String>,
        attributes: Option<CertificateAttributes>,
    ) -> anyhow::Result<()> {
        let Some(certificate_template) = self.get_certificate_template(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Certificate template ('{id}') is not found."
            )));
        };

        self.api
            .db
            .certificates()
            .update_certificate_template(
                user_id,
                &CertificateTemplate {
                    name: if let Some(name) = name {
                        name
                    } else {
                        certificate_template.name
                    },
                    attributes: if let Some(attributes) = attributes {
                        attributes
                    } else {
                        certificate_template.attributes
                    },
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
        format: ExportFormat,
        passphrase: Option<&str>,
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
        Ok(match format {
            ExportFormat::Pem => {
                Self::export_key_pair_to_pem_archive(certificate, private_key, passphrase)?
            }
            ExportFormat::Pkcs8 => Self::export_private_key_to_pkcs8(private_key, passphrase)?,
            ExportFormat::Pkcs12 => Self::export_key_pair_to_pkcs12(
                &certificate_template.name,
                &private_key,
                &certificate,
                passphrase,
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

        log::debug!(
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

            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
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
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with certificates utility.
    pub fn certificates(&self) -> CertificatesApi<DR, ET> {
        CertificatesApi::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api, mock_user, MockResolver},
        utils::{
            CertificateAttributes, CertificatesApi, ExportFormat, ExtendedKeyUsage, KeyUsage,
            PrivateKeyAlgorithm, PrivateKeyEllipticCurve, PrivateKeySize, SignatureAlgorithm,
            Version,
        },
    };
    use insta::assert_debug_snapshot;
    use lettre::transport::stub::AsyncStubTransport;
    use openssl::{hash::MessageDigest, pkcs12::Pkcs12};
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

    #[actix_rt::test]
    async fn can_create_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
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
                    .create_private_key(mock_user.id, format!("pk-{:?}-{:?}", alg, pass), alg, pass)
                    .await?;
                assert_eq!(private_key.alg, alg);

                let imported_key =
                    CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                        &private_key.pkcs8,
                        pass,
                    )?;
                assert_eq!(imported_key.bits(), bits);
            }
        }

        Ok(())
    }

    #[actix_rt::test]
    async fn can_change_private_key_passphrase() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let private_key = certificates
            .create_private_key(mock_user.id, "pk", PrivateKeyAlgorithm::Ed25519, None)
            .await?;

        // Decrypting without password should succeed.
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_ok()
        );

        // Set passphrase.
        certificates
            .update_private_key(mock_user.id, private_key.id, None, None, Some("pass"))
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                None,
            )
            .is_err()
        );
        // Decrypting with passphrase should succeed.
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
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
                None,
                Some("pass"),
                Some("pass-1"),
            )
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
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
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should succeed.
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass-1"),
            )
            .is_ok()
        );

        // Remove passphrase.
        certificates
            .update_private_key(mock_user.id, private_key.id, None, Some("pass-1"), None)
            .await?;

        // Decrypting without passphrase should succeed.
        let private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
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
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should fail.
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &private_key.pkcs8,
                Some("pass-1"),
            )
            .is_err()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_change_private_key_name() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let private_key = certificates
            .create_private_key(
                mock_user.id,
                "pk",
                PrivateKeyAlgorithm::Ed25519,
                Some("pass"),
            )
            .await?;

        // Update name.
        certificates
            .update_private_key(mock_user.id, private_key.id, Some("pk-new"), None, None)
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
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
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
                Some("pk-new-new"),
                Some("pass"),
                Some("pass-1"),
            )
            .await?;

        // Name should change and decrypting with old passphrase should fail.
        let updated_private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(updated_private_key.name, "pk-new-new");
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &updated_private_key.pkcs8,
                Some("pass"),
            )
            .is_err()
        );
        // Decrypting with new passphrase should succeed.
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
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
                Some("pk"),
                Some("pass-1"),
                None,
            )
            .await?;

        // Name should change and decrypting without passphrase should succeed.
        let updated_private_key = certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .unwrap();
        assert_eq!(updated_private_key.name, "pk");
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &updated_private_key.pkcs8,
                None,
            )
            .is_ok()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_export_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create private key without passphrase.
        let certificates = CertificatesApi::new(&api);
        let private_key = certificates
            .create_private_key(mock_user.id, "pk", PrivateKeyAlgorithm::Ed25519, None)
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                ExportFormat::Pkcs8,
                None,
                None,
            )
            .await?;
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8, None,
            )
            .is_ok()
        );
        // Export private key with passphrase and make sure it can be imported with passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                ExportFormat::Pkcs8,
                None,
                Some("pass"),
            )
            .await?;
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        // Set passphrase and repeat.
        certificates
            .update_private_key(mock_user.id, private_key.id, None, None, Some("pass"))
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                ExportFormat::Pkcs8,
                Some("pass"),
                None,
            )
            .await?;
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8, None,
            )
            .is_ok()
        );
        // Export private key with passphrase and make sure it can be imported with passphrase.
        let pkcs8 = certificates
            .export_private_key(
                mock_user.id,
                private_key.id,
                ExportFormat::Pkcs8,
                Some("pass"),
                Some("pass"),
            )
            .await?;
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8,
                Some("pass"),
            )
            .is_ok()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let private_key = certificates
            .create_private_key(mock_user.id, "pk", PrivateKeyAlgorithm::Ed25519, None)
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

        assert!(certificates
            .get_private_key(mock_user.id, private_key.id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_return_multiple_private_keys() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        assert!(certificates
            .get_private_keys(mock_user.id)
            .await?
            .is_empty());

        let private_key_one = certificates
            .create_private_key(mock_user.id, "pk", PrivateKeyAlgorithm::Ed25519, None)
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
            .create_private_key(mock_user.id, "pk-2", PrivateKeyAlgorithm::Ed25519, None)
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

        assert!(certificates
            .get_private_keys(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_create_certificate_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
            .await?;
        assert_eq!(certificate_template.name, "ct");
        assert_eq!(
            certificate_template.attributes,
            get_mock_certificate_attributes()?
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_change_certificate_template_attributes() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
            .await?;

        // Update attributes.
        certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                None,
                Some(CertificateAttributes {
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

    #[actix_rt::test]
    async fn can_change_certificate_template_name() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
            .await?;

        // Update name.
        certificates
            .update_certificate_template(
                mock_user.id,
                certificate_template.id,
                Some("ct-new".to_string()),
                None,
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
                Some("ct-new-new".to_string()),
                Some(CertificateAttributes {
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

    #[actix_rt::test]
    async fn can_remove_certificate_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
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

        assert!(certificates
            .get_certificate_template(mock_user.id, certificate_template.id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_return_multiple_certificate_templates() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        assert!(certificates
            .get_certificate_templates(mock_user.id)
            .await?
            .is_empty());

        let certificate_template_one = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
            .await?;
        assert_eq!(
            certificates.get_certificate_templates(mock_user.id).await?,
            vec![certificate_template_one.clone()]
        );

        let certificate_template_two = certificates
            .create_certificate_template(mock_user.id, "ct-2", get_mock_certificate_attributes()?)
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

        assert!(certificates
            .get_certificate_templates(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[test]
    fn picks_correct_message_digest() -> anyhow::Result<()> {
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
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
                CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha1
                )? == MessageDigest::sha1()
            );
            assert!(
                CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
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
                CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha384
                )? == MessageDigest::sha384()
            );
            assert!(
                CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
                    pk_algorithm,
                    SignatureAlgorithm::Sha512
                )? == MessageDigest::sha512()
            );
        }

        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::get_message_digest(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519
            )? == MessageDigest::null()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn correctly_generates_x509_certificate() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificate_template = api
            .certificates()
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
            .await?;

        let exported_certificate_pair = api
            .certificates()
            .generate_self_signed_certificate(
                mock_user.id,
                certificate_template.id,
                ExportFormat::Pkcs12,
                None,
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

    #[actix_rt::test]
    async fn properly_shares_certificate_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create and share policy.
        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
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

    #[actix_rt::test]
    async fn properly_unshares_certificate_template() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
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

        assert!(api
            .users()
            .get_user_share(template_share_one.id)
            .await?
            .is_none());

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

        assert!(api
            .users()
            .get_user_share(template_share_two.id)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_unshares_certificate_template_when_it_is_removed() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create and share template.
        let certificates = CertificatesApi::new(&api);
        let certificate_template = certificates
            .create_certificate_template(mock_user.id, "ct", get_mock_certificate_attributes()?)
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

        assert!(api
            .users()
            .get_user_share(template_share.id)
            .await?
            .is_none());

        Ok(())
    }
}
