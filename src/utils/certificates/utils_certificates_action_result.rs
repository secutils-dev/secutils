use crate::utils::PrivateKey;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesActionResult {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate(Vec<u8>),
    GetPrivateKeys(Vec<PrivateKey>),
    CreatePrivateKey(PrivateKey),
    ChangePrivateKeyPassphrase,
    RemovePrivateKey,
    ExportPrivateKey(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        PrivateKey, PrivateKeyAlgorithm, PrivateKeySize, UtilsCertificatesActionResult,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsCertificatesActionResult::GenerateSelfSignedCertificate (vec![1,2,3]), @r###"
        {
          "type": "generateSelfSignedCertificate",
          "value": [
            1,
            2,
            3
          ]
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::GetPrivateKeys(vec![PrivateKey {
            name: "pk-name".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            },
            pkcs8: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        }]), @r###"
        {
          "type": "getPrivateKeys",
          "value": [
            {
              "name": "pk-name",
              "alg": {
                "keyType": "rsa",
                "keySize": "2048"
              },
              "pkcs8": [],
              "createdAt": 946720800
            }
          ]
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::CreatePrivateKey(PrivateKey {
            name: "pk-name".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            },
            pkcs8: vec![1, 2, 3],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        }), @r###"
        {
          "type": "createPrivateKey",
          "value": {
            "name": "pk-name",
            "alg": {
              "keyType": "rsa",
              "keySize": "2048"
            },
            "pkcs8": [
              1,
              2,
              3
            ],
            "createdAt": 946720800
          }
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::ChangePrivateKeyPassphrase, @r###"
        {
          "type": "changePrivateKeyPassphrase"
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::RemovePrivateKey, @r###"
        {
          "type": "removePrivateKey"
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::ExportPrivateKey(vec![1, 2, 3]), @r###"
        {
          "type": "exportPrivateKey",
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
