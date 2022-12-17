use crate::utils::{
    PublicKeyAlgorithm, SignatureAlgorithm, UtilsCertificatesRequest, UtilsCertificatesResponse,
};
use anyhow::anyhow;
use openssl::{
    asn1::Asn1Time,
    bn::{BigNum, MsbOption},
    dsa::Dsa,
    ec::{EcGroup, EcKey},
    hash::MessageDigest,
    nid::Nid,
    pkey::PKey,
    rsa::Rsa,
    x509::{
        extension::{BasicConstraints, KeyUsage, SubjectKeyIdentifier},
        X509NameBuilder, X509,
    },
};

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
    pk_alg: &PublicKeyAlgorithm,
    sig_alg: &SignatureAlgorithm,
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

pub struct UtilsCertificatesExecutor {}
impl UtilsCertificatesExecutor {
    pub async fn execute(
        request: UtilsCertificatesRequest,
    ) -> anyhow::Result<UtilsCertificatesResponse> {
        match request {
            UtilsCertificatesRequest::GenerateCa {
                common_name,
                country,
                state_or_province,
                locality,
                organization,
                organizational_unit,
                version,
                public_key_algorithm,
                signature_algorithm,
                not_valid_before,
                not_valid_after,
            } => {
                let mut x509_name = X509NameBuilder::new()?;
                set_name_attribute(&mut x509_name, "CN", &common_name)?;
                set_name_attribute(&mut x509_name, "C", &country)?;
                set_name_attribute(&mut x509_name, "ST", &state_or_province)?;
                set_name_attribute(&mut x509_name, "L", &locality)?;
                set_name_attribute(&mut x509_name, "O", &organization)?;
                set_name_attribute(&mut x509_name, "OU", &organizational_unit)?;
                let x509_name = x509_name.build();

                let key_pair = match public_key_algorithm {
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

                let mut x509 = X509::builder()?;
                x509.set_subject_name(&x509_name)?;
                x509.set_issuer_name(&x509_name)?;
                x509.set_version((version - 1) as i32)?;

                let serial_number = {
                    let mut serial = BigNum::new()?;
                    serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
                    serial.to_asn1_integer()?
                };
                x509.set_serial_number(&serial_number)?;

                x509.set_pubkey(&key_pair)?;
                let not_before = Asn1Time::from_unix(not_valid_before.unix_timestamp())?;
                x509.set_not_before(&not_before)?;
                let not_after = Asn1Time::from_unix(not_valid_after.unix_timestamp())?;
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
                    &key_pair,
                    message_digest(&public_key_algorithm, &signature_algorithm)?,
                )?;
                let cert = x509.build();

                Ok(UtilsCertificatesResponse::GenerateCA {
                    private_key: key_pair.private_key_to_pem_pkcs8()?,
                    public_key: key_pair.public_key_to_pem()?,
                    cert: cert.to_pem()?,
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
