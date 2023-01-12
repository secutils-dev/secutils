use crate::{
    api::Api,
    users::{User, UserDataType},
    utils::{
        CertificateFormat, PublicKeyAlgorithm, SelfSignedCertificate, SignatureAlgorithm,
        UtilsCertificatesRequest, UtilsCertificatesResponse,
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
    x509::{
        extension::{BasicConstraints, KeyUsage, SubjectKeyIdentifier},
        X509NameBuilder, X509,
    },
};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

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
    pk_alg: PublicKeyAlgorithm,
    sig_alg: SignatureAlgorithm,
) -> anyhow::Result<MessageDigest> {
    match (pk_alg, sig_alg) {
        (PublicKeyAlgorithm::Rsa, SignatureAlgorithm::Md5) => Ok(MessageDigest::md5()),
        (
            PublicKeyAlgorithm::Rsa | PublicKeyAlgorithm::Dsa | PublicKeyAlgorithm::Ecdsa,
            SignatureAlgorithm::Sha1,
        ) => Ok(MessageDigest::sha1()),
        (
            PublicKeyAlgorithm::Rsa | PublicKeyAlgorithm::Dsa | PublicKeyAlgorithm::Ecdsa,
            SignatureAlgorithm::Sha256,
        ) => Ok(MessageDigest::sha256()),
        (PublicKeyAlgorithm::Rsa | PublicKeyAlgorithm::Ecdsa, SignatureAlgorithm::Sha384) => {
            Ok(MessageDigest::sha384())
        }
        (PublicKeyAlgorithm::Rsa | PublicKeyAlgorithm::Ecdsa, SignatureAlgorithm::Sha512) => {
            Ok(MessageDigest::sha512())
        }
        (PublicKeyAlgorithm::Ed25519, SignatureAlgorithm::Ed25519) => Ok(MessageDigest::null()),
        _ => Err(anyhow!(
            "Public key ({:?}) and signature ({:?}) algorithms are not compatible",
            pk_alg,
            sig_alg
        )),
    }
}

fn convert_to_pem_archive(
    certificate: X509,
    key_pair: PKey<Private>,
    passphrase: Option<String>,
) -> anyhow::Result<Vec<u8>> {
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

fn convert_to_pkcs12(
    certificate_template: SelfSignedCertificate,
    certificate: X509,
    key_pair: PKey<Private>,
    passphrase: Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let pkcs12_builder = Pkcs12::builder();
    let pkcs12 = pkcs12_builder
        .build(
            &passphrase.unwrap_or_default(),
            &certificate_template.name,
            &key_pair,
            &certificate,
        )
        .with_context(|| "Cannot build PKCS12 certificate bundle.")?;

    pkcs12
        .to_der()
        .with_context(|| "Cannot convert PKCS12 certificate bundle to DER.")
}

fn generate_key(public_key_algorithm: PublicKeyAlgorithm) -> anyhow::Result<PKey<Private>> {
    let private_key = match public_key_algorithm {
        PublicKeyAlgorithm::Rsa => {
            let rsa = Rsa::generate(2048)?;
            PKey::from_rsa(rsa)?
        }
        PublicKeyAlgorithm::Dsa => {
            let dsa = Dsa::generate(2048)?;
            PKey::from_dsa(dsa)?
        }
        PublicKeyAlgorithm::Ecdsa => {
            let ec_group = EcGroup::from_curve_name(Nid::SECP256K1)?;
            PKey::from_ec_key(EcKey::generate(&ec_group)?)?
        }
        PublicKeyAlgorithm::Ed25519 => PKey::generate_ed25519()?,
    };

    Ok(private_key)
}

pub struct UtilsCertificatesExecutor;
impl UtilsCertificatesExecutor {
    pub async fn execute(
        user: User,
        api: &Api,
        request: UtilsCertificatesRequest,
    ) -> anyhow::Result<UtilsCertificatesResponse> {
        match request {
            UtilsCertificatesRequest::GenerateSelfSignedCertificate {
                template_name,
                format,
                passphrase,
            } => {
                let certificate_templates = api
                    .users()
                    .get_data::<BTreeMap<String, SelfSignedCertificate>>(
                        user.id,
                        UserDataType::SelfSignedCertificates,
                    )
                    .await?;

                let certificate_template = certificate_templates
                    .and_then(|mut map| map.remove(&template_name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find self-signed certificate with name: {}",
                            template_name
                        )
                    })?;

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

                let private_key = generate_key(certificate_template.public_key_algorithm)?;

                let mut x509 = X509::builder()?;
                x509.set_subject_name(&x509_name)?;
                x509.set_issuer_name(&x509_name)?;
                x509.set_version((certificate_template.version - 1) as i32)?;

                let serial_number = {
                    let mut serial = BigNum::new()?;
                    serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
                    serial.to_asn1_integer()?
                };
                x509.set_serial_number(&serial_number)?;

                x509.set_pubkey(&private_key)?;
                let not_before =
                    Asn1Time::from_unix(certificate_template.not_valid_before.unix_timestamp())?;
                x509.set_not_before(&not_before)?;
                let not_after =
                    Asn1Time::from_unix(certificate_template.not_valid_after.unix_timestamp())?;
                x509.set_not_after(&not_after)?;

                x509.append_extension(BasicConstraints::new().critical().ca().build()?)?;
                x509.append_extension(
                    KeyUsage::new()
                        .critical()
                        .key_cert_sign()
                        .crl_sign()
                        .build()?,
                )?;

                let subject_key_identifier =
                    SubjectKeyIdentifier::new().build(&x509.x509v3_context(None, None))?;
                x509.append_extension(subject_key_identifier)?;

                x509.sign(
                    &private_key,
                    message_digest(
                        certificate_template.public_key_algorithm,
                        certificate_template.signature_algorithm,
                    )?,
                )?;

                let x509_certificate = x509.build();
                let certificate = match format {
                    CertificateFormat::Pem => {
                        convert_to_pem_archive(x509_certificate, private_key, passphrase)?
                    }
                    CertificateFormat::Pkcs12 => convert_to_pkcs12(
                        certificate_template,
                        x509_certificate,
                        private_key,
                        passphrase,
                    )?,
                };

                log::info!("Serialized certificate ({} bytes).", certificate.len());

                Ok(UtilsCertificatesResponse::GenerateSelfSignedCertificate {
                    format,
                    certificate,
                })
            }
            UtilsCertificatesRequest::GenerateRsaKeyPair => {
                let rsa = Rsa::generate(2048)?;
                let public_pem = rsa.public_key_to_pem()?;

                Ok(UtilsCertificatesResponse::GenerateRsaKeyPair(public_pem))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::generate_key;
    use crate::utils::PublicKeyAlgorithm;

    #[test]
    fn key_generation() -> anyhow::Result<()> {
        let rsa_key = generate_key(PublicKeyAlgorithm::Rsa)?;
        let rsa_key = rsa_key.rsa()?;

        assert!(rsa_key.check_key()?);
        assert_eq!(rsa_key.size(), 256);

        let dsa_key = generate_key(PublicKeyAlgorithm::Dsa)?;
        let dsa_key = dsa_key.dsa()?;

        assert_eq!(dsa_key.size(), 72);

        let ecdsa_key = generate_key(PublicKeyAlgorithm::Ecdsa)?;
        let ecdsa_key = ecdsa_key.ec_key()?;

        ecdsa_key.check_key()?;

        let ed25519_key = generate_key(PublicKeyAlgorithm::Ed25519)?;
        assert_eq!(ed25519_key.bits(), 253);

        Ok(())
    }
}
