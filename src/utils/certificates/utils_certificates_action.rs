use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{PublicUserDataNamespace, User},
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, CertificateFormat, ExtendedKeyUsage,
        KeyAlgorithm, KeyUsage, SelfSignedCertificate, SignatureAlgorithm,
        UtilsCertificatesActionResult,
    },
};
use anyhow::{anyhow, Context};
use openssl::{
    asn1::Asn1Time,
    bn::{BigNum, MsbOption},
    dsa::Dsa,
    ec::{EcGroup, EcKey},
    hash::MessageDigest,
    nid::Nid,
    pkcs12::Pkcs12,
    pkey::{PKey, Private},
    rsa::Rsa,
    symm::Cipher,
    x509::{extension, X509NameBuilder, X509},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesAction {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        template_name: String,
        format: CertificateFormat,
        passphrase: Option<String>,
    },
    GenerateRsaKeyPair,
}

impl UtilsCertificatesAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsCertificatesAction::GenerateSelfSignedCertificate { template_name, .. } => {
                if template_name.is_empty() {
                    anyhow::bail!("Template name cannot be empty");
                }

                if template_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Template name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
            UtilsCertificatesAction::GenerateRsaKeyPair => {}
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsCertificatesActionResult> {
        match self {
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name,
                format,
                passphrase,
            } => {
                let certificate_template = api
                    .users()
                    .get_data::<BTreeMap<String, SelfSignedCertificate>>(
                        user.id,
                        PublicUserDataNamespace::SelfSignedCertificates,
                    )
                    .await?
                    .and_then(|mut map| map.value.remove(&template_name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find self-signed certificate with name: {}",
                            template_name
                        )
                    })?;

                let key = generate_key(certificate_template.key_algorithm)?;
                let certificate = match format {
                    CertificateFormat::Pem => {
                        convert_to_pem_archive(certificate_template, key, passphrase)?
                    }
                    CertificateFormat::Pkcs8 => convert_to_pkcs8(key, passphrase)?,
                    CertificateFormat::Pkcs12 => {
                        convert_to_pkcs12(certificate_template, key, passphrase)?
                    }
                };

                log::info!("Serialized certificate ({} bytes).", certificate.len());

                Ok(
                    UtilsCertificatesActionResult::GenerateSelfSignedCertificate {
                        format,
                        certificate,
                    },
                )
            }
            UtilsCertificatesAction::GenerateRsaKeyPair => {
                let rsa = Rsa::generate(2048)?;
                let public_pem = rsa.public_key_to_pem()?;

                Ok(UtilsCertificatesActionResult::GenerateRsaKeyPair(
                    public_pem,
                ))
            }
        }
    }
}

fn set_name_attribute(
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

fn message_digest(
    pk_alg: KeyAlgorithm,
    sig_alg: SignatureAlgorithm,
) -> anyhow::Result<MessageDigest> {
    match (pk_alg, sig_alg) {
        (KeyAlgorithm::Rsa { .. }, SignatureAlgorithm::Md5) => Ok(MessageDigest::md5()),
        (
            KeyAlgorithm::Rsa { .. } | KeyAlgorithm::Dsa { .. } | KeyAlgorithm::Ecdsa { .. },
            SignatureAlgorithm::Sha1,
        ) => Ok(MessageDigest::sha1()),
        (
            KeyAlgorithm::Rsa { .. } | KeyAlgorithm::Dsa { .. } | KeyAlgorithm::Ecdsa { .. },
            SignatureAlgorithm::Sha256,
        ) => Ok(MessageDigest::sha256()),
        (KeyAlgorithm::Rsa { .. } | KeyAlgorithm::Ecdsa { .. }, SignatureAlgorithm::Sha384) => {
            Ok(MessageDigest::sha384())
        }
        (KeyAlgorithm::Rsa { .. } | KeyAlgorithm::Ecdsa { .. }, SignatureAlgorithm::Sha512) => {
            Ok(MessageDigest::sha512())
        }
        (KeyAlgorithm::Ed25519, SignatureAlgorithm::Ed25519) => Ok(MessageDigest::null()),
        _ => Err(anyhow!(
            "Public key ({:?}) and signature ({:?}) algorithms are not compatible",
            pk_alg,
            sig_alg
        )),
    }
}

fn convert_to_pem_archive(
    certificate_template: SelfSignedCertificate,
    key_pair: PKey<Private>,
    passphrase: Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let certificate = generate_x509_certificate(&certificate_template, &key_pair)?;

    // 64kb should be more than enough for the certificate + private key.
    let mut zip_buffer = [0; 65536];
    let size = {
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_buffer[..]));

        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("certificate.crt", options)?;
        zip.write_all(&certificate.to_pem()?)?;

        zip.start_file("private_key.key", options)?;
        zip.write_all(&match passphrase {
            None => key_pair.private_key_to_pem_pkcs8()?,
            Some(passphrase) => key_pair.private_key_to_pem_pkcs8_passphrase(
                Cipher::aes_128_cbc(),
                passphrase.as_bytes(),
            )?,
        })?;

        zip.finish()?.position() as usize
    };

    Ok(zip_buffer[..size].to_vec())
}

fn convert_to_pkcs8(
    key_pair: PKey<Private>,
    passphrase: Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let pkcs8 = if let Some(passphrase) = passphrase {
        // AEAD ciphers not supported in this command.
        key_pair.private_key_to_pkcs8_passphrase(Cipher::aes_128_cbc(), passphrase.as_bytes())
    } else {
        key_pair.private_key_to_pkcs8()
    };

    pkcs8.with_context(|| "Cannot convert private key to PKCS8.")
}

fn convert_to_pkcs12(
    certificate_template: SelfSignedCertificate,
    key_pair: PKey<Private>,
    passphrase: Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let certificate = generate_x509_certificate(&certificate_template, &key_pair)?;

    let mut pkcs12_builder = Pkcs12::builder();
    let pkcs12 = pkcs12_builder
        .name(&certificate_template.name)
        .pkey(&key_pair)
        .cert(&certificate)
        .build2(&passphrase.unwrap_or_default())
        .with_context(|| "Cannot build PKCS12 certificate bundle.")?;

    pkcs12
        .to_der()
        .with_context(|| "Cannot convert PKCS12 certificate bundle to DER.")
}

fn generate_key(public_key_algorithm: KeyAlgorithm) -> anyhow::Result<PKey<Private>> {
    let private_key = match public_key_algorithm {
        KeyAlgorithm::Rsa { key_size } => {
            let rsa = Rsa::generate(key_size as u32)?;
            PKey::from_rsa(rsa)?
        }
        KeyAlgorithm::Dsa { key_size } => {
            let dsa = Dsa::generate(key_size as u32)?;
            PKey::from_dsa(dsa)?
        }
        KeyAlgorithm::Ecdsa { curve } => {
            let ec_group = EcGroup::from_curve_name(Nid::from_raw(curve as i32))?;
            PKey::from_ec_key(EcKey::generate(&ec_group)?)?
        }
        KeyAlgorithm::Ed25519 => PKey::generate_ed25519()?,
    };

    Ok(private_key)
}

fn generate_x509_certificate(
    certificate_template: &SelfSignedCertificate,
    key: &PKey<Private>,
) -> anyhow::Result<X509> {
    let mut x509_name = X509NameBuilder::new()?;
    set_name_attribute(&mut x509_name, "CN", &certificate_template.common_name)?;
    set_name_attribute(&mut x509_name, "C", &certificate_template.country)?;
    set_name_attribute(
        &mut x509_name,
        "ST",
        &certificate_template.state_or_province,
    )?;
    set_name_attribute(&mut x509_name, "L", &certificate_template.locality)?;
    set_name_attribute(&mut x509_name, "O", &certificate_template.organization)?;
    set_name_attribute(
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

    x509.set_pubkey(key)?;
    let not_before = Asn1Time::from_unix(certificate_template.not_valid_before.unix_timestamp())?;
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

    x509.sign(
        key,
        message_digest(
            certificate_template.key_algorithm,
            certificate_template.signature_algorithm,
        )?,
    )?;

    Ok(x509.build())
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        certificates::utils_certificates_action::{
            generate_key, generate_x509_certificate, message_digest,
        },
        tests::MockSelfSignedCertificate,
        CertificateFormat, EllipticCurve, KeyAlgorithm, KeySize, SignatureAlgorithm,
        UtilsCertificatesAction, Version,
    };
    use insta::assert_debug_snapshot;
    use openssl::hash::MessageDigest;
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pem" }
}
          "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: CertificateFormat::Pem,
                passphrase: None,
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "generateSelfSignedCertificate",
    "value": { "templateName": "template", "format": "pkcs12", "passphrase": "phrase" }
}
          "#
            )?,
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "template".to_string(),
                format: CertificateFormat::Pkcs12,
                passphrase: Some("phrase".to_string()),
            }
        );
        assert_eq!(
            serde_json::from_str::<UtilsCertificatesAction>(
                r#"
{
    "type": "generateRsaKeyPair"
}
          "#
            )?,
            UtilsCertificatesAction::GenerateRsaKeyPair
        );

        Ok(())
    }

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsCertificatesAction::GenerateRsaKeyPair
            .validate()
            .is_ok());

        assert!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(100),
            format: CertificateFormat::Pem,
            passphrase: None,
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: CertificateFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Template name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "a".repeat(101),
            format: CertificateFormat::Pem,
            passphrase: None,
        }.validate(), @r###"
        Err(
            "Template name cannot be longer than 100 characters",
        )
        "###);

        Ok(())
    }

    #[test]
    fn correctly_generate_keys() -> anyhow::Result<()> {
        let rsa_key = generate_key(KeyAlgorithm::Rsa {
            key_size: KeySize::Size1024,
        })?;
        let rsa_key = rsa_key.rsa()?;

        assert!(rsa_key.check_key()?);
        assert_eq!(rsa_key.size(), 128);

        let dsa_key = generate_key(KeyAlgorithm::Dsa {
            key_size: KeySize::Size2048,
        })?;
        let dsa_key = dsa_key.dsa()?;

        assert_eq!(dsa_key.size(), 72);

        let ecdsa_key = generate_key(KeyAlgorithm::Ecdsa {
            curve: EllipticCurve::SECP256R1,
        })?;
        let ecdsa_key = ecdsa_key.ec_key()?;

        ecdsa_key.check_key()?;

        let ed25519_key = generate_key(KeyAlgorithm::Ed25519)?;
        assert_eq!(ed25519_key.bits(), 256);

        Ok(())
    }

    #[test]
    fn picks_correct_message_digest() -> anyhow::Result<()> {
        assert!(
            message_digest(
                KeyAlgorithm::Rsa {
                    key_size: KeySize::Size1024,
                },
                SignatureAlgorithm::Md5
            )? == MessageDigest::md5()
        );

        for pk_algorithm in [
            KeyAlgorithm::Rsa {
                key_size: KeySize::Size1024,
            },
            KeyAlgorithm::Dsa {
                key_size: KeySize::Size2048,
            },
            KeyAlgorithm::Ecdsa {
                curve: EllipticCurve::SECP256R1,
            },
        ] {
            assert!(
                message_digest(pk_algorithm, SignatureAlgorithm::Sha1)? == MessageDigest::sha1()
            );
            assert!(
                message_digest(pk_algorithm, SignatureAlgorithm::Sha256)?
                    == MessageDigest::sha256()
            );
        }

        for pk_algorithm in [
            KeyAlgorithm::Rsa {
                key_size: KeySize::Size1024,
            },
            KeyAlgorithm::Ecdsa {
                curve: EllipticCurve::SECP256R1,
            },
        ] {
            assert!(
                message_digest(pk_algorithm, SignatureAlgorithm::Sha384)?
                    == MessageDigest::sha384()
            );
            assert!(
                message_digest(pk_algorithm, SignatureAlgorithm::Sha512)?
                    == MessageDigest::sha512()
            );
        }

        assert!(
            message_digest(KeyAlgorithm::Ed25519, SignatureAlgorithm::Ed25519)?
                == MessageDigest::null()
        );

        Ok(())
    }

    #[test]
    fn correctly_generates_x509_certificate() -> anyhow::Result<()> {
        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        let certificate_template = MockSelfSignedCertificate::new(
            "test-1-name",
            KeyAlgorithm::Rsa {
                key_size: KeySize::Size1024,
            },
            SignatureAlgorithm::Sha256,
            not_valid_before,
            not_valid_after,
            Version::One,
        )
        .build();
        let key = generate_key(KeyAlgorithm::Rsa {
            key_size: KeySize::Size1024,
        })?;

        let x509_certificate = generate_x509_certificate(&certificate_template, &key)?;

        assert_debug_snapshot!(x509_certificate.not_before(), @"Jan  1 10:00:00 2000 GMT");
        assert_debug_snapshot!(x509_certificate.not_after(), @"Jan  1 10:00:00 2010 GMT");
        assert_eq!(
            x509_certificate.public_key()?.public_key_to_der()?,
            key.public_key_to_der()?
        );

        Ok(())
    }
}
