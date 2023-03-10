use crate::utils::CertificateFormat;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesActionResult {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        certificate: Vec<u8>,
        format: CertificateFormat,
    },
    GenerateRsaKeyPair(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use crate::utils::{CertificateFormat, UtilsCertificatesActionResult};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsCertificatesActionResult::GenerateSelfSignedCertificate {
            certificate: vec![1,2,3],
            format: CertificateFormat::Pem
        }, @r###"
        {
          "type": "generateSelfSignedCertificate",
          "value": {
            "certificate": [
              1,
              2,
              3
            ],
            "format": "pem"
          }
        }
        "###);
        assert_json_snapshot!(UtilsCertificatesActionResult::GenerateRsaKeyPair(vec![1,2,3]), @r###"
        {
          "type": "generateRsaKeyPair",
          "value": [
            1,
            2,
            3
          ]
        }
        "###);

        Ok(())
    }
}
