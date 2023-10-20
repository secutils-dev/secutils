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
    UpdatePrivateKey,
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
    use uuid::uuid;
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
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "pk-name".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            },
            pkcs8: vec![],
            encrypted: true,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        }]), @r###"
        {
          "type": "getPrivateKeys",
          "value": [
            {
              "id": "00000000-0000-0000-0000-000000000001",
              "name": "pk-name",
              "alg": {
                "keyType": "rsa",
                "keySize": "2048"
              },
              "pkcs8": [],
              "encrypted": true,
              "createdAt": 946720800
            }
          ]
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::CreatePrivateKey(PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "pk-name".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048
            },
            pkcs8: vec![1, 2, 3],
            encrypted: false,
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        }), @r###"
        {
          "type": "createPrivateKey",
          "value": {
            "id": "00000000-0000-0000-0000-000000000001",
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
            "encrypted": false,
            "createdAt": 946720800
          }
        }
        "###);

        assert_json_snapshot!(UtilsCertificatesActionResult::UpdatePrivateKey, @r###"
        {
          "type": "updatePrivateKey"
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
