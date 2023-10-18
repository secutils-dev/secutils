use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{PublicUserDataNamespace, UserId},
    utils::{
        CertificateTemplate, ExportFormat, ExtendedKeyUsage, KeyUsage, PrivateKey,
        PrivateKeyAlgorithm, SignatureAlgorithm,
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
    collections::BTreeMap,
    io::{Cursor, Write},
    time::Instant,
};
use time::OffsetDateTime;
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

    /// Retrieves the private key with the specified name.
    pub async fn get_private_key(
        &self,
        user_id: UserId,
        name: &str,
    ) -> anyhow::Result<Option<PrivateKey>> {
        self.api
            .db
            .certificates()
            .get_private_key(user_id, name)
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

    /// Updates private key passphrase.
    pub async fn change_private_key_passphrase(
        &self,
        user_id: UserId,
        name: &str,
        passphrase: Option<&str>,
        new_passphrase: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(private_key) = self.get_private_key(user_id, name).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{name}') is not found."
            )));
        };

        // Try to decrypt private key using the provided passphrase.
        let pkcs8_private_key = Self::import_private_key_from_pkcs8(&private_key.pkcs8, passphrase)
            .map_err(|err| {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Unable to decrypt private key ('{name}') with the provided passphrase."
                )))
            })?;

        // Convert private key to PKCS8 using the new passphrase, and update it in the database.
        self.api
            .db
            .certificates()
            .update_private_key(
                user_id,
                &PrivateKey {
                    pkcs8: Self::export_private_key_to_pkcs8(pkcs8_private_key, new_passphrase)?,
                    encrypted: new_passphrase.is_some(),
                    ..private_key
                },
            )
            .await
    }

    /// Removes private key with the specified name.
    pub async fn remove_private_key(&self, user_id: UserId, name: &str) -> anyhow::Result<()> {
        self.api
            .db
            .certificates()
            .remove_private_key(user_id, name)
            .await
    }

    /// Exports private key with the specified name to the specified format and passphrase.
    pub async fn export_private_key(
        &self,
        user_id: UserId,
        name: &str,
        format: ExportFormat,
        passphrase: Option<&str>,
        export_passphrase: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let Some(private_key) = self.get_private_key(user_id, name).await? else {
            bail!(SecutilsError::client(format!(
                "Private key ('{name}') is not found."
            )));
        };

        // Try to decrypt private key using the provided passphrase.
        let pkcs8_private_key = Self::import_private_key_from_pkcs8(&private_key.pkcs8, passphrase)
            .map_err(|err| {
                SecutilsError::client_with_root_cause(anyhow!(err).context(format!(
                    "Unable to decrypt private key ('{name}') with the provided passphrase."
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
                "Unable to export private key ('{name}') to the specified format ('{format:?}')."
            )))
            .into()
        })
    }

    /// Retrieves all private keys that belong to the specified user.
    pub async fn get_private_keys(&self, user_id: UserId) -> anyhow::Result<Vec<PrivateKey>> {
        self.api.db.certificates().get_private_keys(user_id).await
    }

    /// Generates private key and certificate pair.
    pub async fn generate_self_signed_certificate(
        &self,
        user_id: UserId,
        template_name: &str,
        format: ExportFormat,
        passphrase: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        // Extract certificate template.
        let certificate_template = self
            .api
            .users()
            .get_data::<BTreeMap<String, CertificateTemplate>>(
                user_id,
                PublicUserDataNamespace::CertificateTemplates,
            )
            .await?
            .and_then(|mut map| map.value.remove(template_name))
            .ok_or_else(|| {
                SecutilsError::client(format!(
                    "Certificate template ('{template_name}') is not found."
                ))
            })?;

        // Create X509 certificate builder pre-filled with the specified template properties.
        let mut certificate_builder = Self::create_x509_certificate_builder(&certificate_template)?;

        // Generate private key, set certificate public key and sign it.
        let private_key = Self::generate_private_key(certificate_template.key_algorithm)?;
        certificate_builder.set_pubkey(&private_key)?;
        certificate_builder.sign(
            &private_key,
            Self::get_message_digest(
                certificate_template.key_algorithm,
                certificate_template.signature_algorithm,
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
        Self::set_x509_name_attribute(&mut x509_name, "CN", &certificate_template.common_name)?;
        Self::set_x509_name_attribute(&mut x509_name, "C", &certificate_template.country)?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "ST",
            &certificate_template.state_or_province,
        )?;
        Self::set_x509_name_attribute(&mut x509_name, "L", &certificate_template.locality)?;
        Self::set_x509_name_attribute(&mut x509_name, "O", &certificate_template.organization)?;
        Self::set_x509_name_attribute(
            &mut x509_name,
            "OU",
            &certificate_template.organizational_unit,
        )?;
        let x509_name = x509_name.build();

        let mut x509 = X509::builder()?;
        x509.set_subject_name(&x509_name)?;
        x509.set_issuer_name(&x509_name)?;
        x509.set_version(certificate_template.version.value())?;

        let mut basic_constraint = extension::BasicConstraints::new();
        if certificate_template.is_ca {
            basic_constraint.ca();
        }
        x509.append_extension(basic_constraint.critical().build()?)?;

        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        x509.set_serial_number(&serial_number)?;

        let not_before =
            Asn1Time::from_unix(certificate_template.not_valid_before.unix_timestamp())?;
        x509.set_not_before(&not_before)?;
        let not_after = Asn1Time::from_unix(certificate_template.not_valid_after.unix_timestamp())?;
        x509.set_not_after(&not_after)?;

        if let Some(ref key_usage) = certificate_template.key_usage {
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

        if let Some(ref key_usage) = certificate_template.extended_key_usage {
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
        tests::{mock_api, mock_user, MockCertificateTemplate, MockResolver},
        users::{DictionaryDataUserDataSetter, PublicUserDataNamespace, UserData},
        utils::{
            CertificatesApi, ExportFormat, PrivateKeyAlgorithm, PrivateKeyEllipticCurve,
            PrivateKeySize, SignatureAlgorithm, Version,
        },
    };
    use insta::assert_debug_snapshot;
    use lettre::transport::stub::AsyncStubTransport;
    use openssl::{hash::MessageDigest, pkcs12::Pkcs12};
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

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
            .change_private_key_passphrase(mock_user.id, "pk", None, Some("pass"))
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, "pk")
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
            .change_private_key_passphrase(mock_user.id, "pk", Some("pass"), Some("pass-1"))
            .await?;

        // Decrypting without passphrase should fail.
        let private_key = certificates
            .get_private_key(mock_user.id, "pk")
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
            .get_private_key(mock_user.id, "pk")
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
            .change_private_key_passphrase(mock_user.id, "pk", Some("pass-1"), None)
            .await?;

        // Decrypting without passphrase should succeed.
        let private_key = certificates
            .get_private_key(mock_user.id, "pk")
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
            .get_private_key(mock_user.id, "pk")
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
    async fn can_export_private_key() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Create private key without passphrase.
        let certificates = CertificatesApi::new(&api);
        certificates
            .create_private_key(mock_user.id, "pk", PrivateKeyAlgorithm::Ed25519, None)
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(mock_user.id, "pk", ExportFormat::Pkcs8, None, None)
            .await?;
        assert!(
            CertificatesApi::<MockResolver, AsyncStubTransport>::import_private_key_from_pkcs8(
                &pkcs8, None,
            )
            .is_ok()
        );
        // Export private key with passphrase and make sure it can be imported with passphrase.
        let pkcs8 = certificates
            .export_private_key(mock_user.id, "pk", ExportFormat::Pkcs8, None, Some("pass"))
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
            .change_private_key_passphrase(mock_user.id, "pk", None, Some("pass"))
            .await?;

        // Export private key without passphrase and make sure it can be without passphrase.
        let pkcs8 = certificates
            .export_private_key(mock_user.id, "pk", ExportFormat::Pkcs8, Some("pass"), None)
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
                "pk",
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
                .get_private_key(mock_user.id, "pk")
                .await?
                .unwrap()
        );

        certificates.remove_private_key(mock_user.id, "pk").await?;

        assert!(certificates
            .get_private_key(mock_user.id, "pk")
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
            vec![private_key_one, private_key_two]
        );

        certificates.remove_private_key(mock_user.id, "pk").await?;
        certificates
            .remove_private_key(mock_user.id, "pk-2")
            .await?;

        assert!(certificates
            .get_private_keys(mock_user.id)
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

        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        // Store certificate.
        let certificate_template = MockCertificateTemplate::new(
            "test-1-name",
            PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size1024,
            },
            SignatureAlgorithm::Sha256,
            not_valid_before,
            not_valid_after,
            Version::One,
        )
        .build();
        DictionaryDataUserDataSetter::upsert(
            &api.db,
            PublicUserDataNamespace::CertificateTemplates,
            UserData::new(
                mock_user.id,
                [(
                    certificate_template.name.clone(),
                    Some(certificate_template.clone()),
                )]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await?;

        let exported_certificate_pair = api
            .certificates()
            .generate_self_signed_certificate(
                mock_user.id,
                &certificate_template.name,
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
}
