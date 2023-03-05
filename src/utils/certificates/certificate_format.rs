use serde::{Deserialize, Serialize};

/// Defines a format to use for the generated certificate(s) and keys.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CertificateFormat {
    /// The PEM format is the most common format that Certificate Authorities issue certificates in.
    /// PEM certificates usually have extensions such as ".pem", ".crt", ".cer", and ".key". They
    /// are Base64 encoded ASCII files and contain "-----BEGIN CERTIFICATE-----" and
    /// "-----END CERTIFICATE-----" statements. Server certificates, intermediate certificates, and
    /// private keys can all be put into the PEM format.
    Pem,
    /// PKCS #12 defines an archive file format for storing many cryptography objects as a single
    /// file. It is commonly used to bundle a private key with its X.509 certificate or to bundle
    /// all the members of a chain of trust. A PKCS #12 file may be encrypted and signed. The
    /// internal storage containers, called "SafeBags", may also be encrypted and signed. A few
    /// SafeBags are predefined to store certificates, private keys and CRLs. PKCS #12 is one of the
    /// family of standards called public-Key Cryptography Standards (PKCS) published by RSA
    /// Laboratories. The filename extension for PKCS #12 files is ".p12" or ".pfx". PKCS#2 format
    /// is the preferred method for transporting private key and its public certificate chains.
    /// The underlying password-based encryption methods is PKCS #5 v2.1. As a general rule,
    /// PKCS12 is the best way to transport and exchange private keys because of the stronger
    /// encryption that it uses to encrypt the private key.
    Pkcs12,
}

#[cfg(test)]
mod tests {
    use crate::utils::CertificateFormat;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(CertificateFormat::Pem, @r###""pem""###);
        assert_json_snapshot!(CertificateFormat::Pkcs12, @r###""pkcs12""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<CertificateFormat>(r###""pem""###)?,
            CertificateFormat::Pem
        );
        assert_eq!(
            serde_json::from_str::<CertificateFormat>(r###""pkcs12""###)?,
            CertificateFormat::Pkcs12
        );

        Ok(())
    }
}
