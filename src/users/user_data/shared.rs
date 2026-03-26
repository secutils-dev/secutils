use crate::users::{EntityTag, secrets::UserSecret};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// The current data file format version. Both export and import use this constant.
pub const DATA_FILE_VERSION: u32 = 1;

/// Represents a secret as it appears in a data file (export or import).
///
/// Both the export writer and the import reader use this struct so the shape is defined once.
/// `encrypted_value` is omitted from serialized output when absent but accepted as missing during
/// deserialization (treated as `None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFileSecret {
    pub id: Uuid,
    pub name: String,
    /// Base64-encoded passphrase-encrypted value, or `None` if values are not included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<EntityTag>,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}

impl DataFileSecret {
    /// Creates a `DataFileSecret` without an encrypted value (name-only export).
    pub fn from_secret(secret: UserSecret) -> Self {
        Self {
            id: secret.id,
            name: secret.name,
            encrypted_value: None,
            tags: secret.tags,
            created_at: secret.created_at,
            updated_at: secret.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DataFileSecret;
    use crate::users::{EntityTag, secrets::UserSecret};
    use insta::assert_json_snapshot;
    use time::macros::datetime;
    use uuid::{Uuid, uuid};

    #[test]
    fn export_secret_from_user_secret() {
        let exported = DataFileSecret::from_secret(UserSecret {
            id: Uuid::nil(),
            user_id: crate::users::UserId::new(),
            name: "MY_SECRET".to_string(),
            encrypted_value: Some(vec![1, 2, 3]),
            tags: vec![EntityTag {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tag".to_string(),
                color: "#color".to_string(),
            }],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-06-01 00:00:00 UTC),
        });
        assert_json_snapshot!(exported, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000000",
          "name": "MY_SECRET",
          "tags": [
            {
              "id": "00000000-0000-0000-0000-000000000001",
              "name": "tag",
              "color": "#color"
            }
          ],
          "createdAt": 1577836800,
          "updatedAt": 1590969600
        }
        "###);
    }

    #[test]
    fn serialize_exported_secret_with_encrypted_value() {
        let secret = DataFileSecret {
            id: Uuid::nil(),
            name: "MY_SECRET".to_string(),
            encrypted_value: Some("base64data".to_string()),
            tags: vec![],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-06-01 00:00:00 UTC),
        };
        assert_json_snapshot!(secret, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000000",
          "name": "MY_SECRET",
          "encryptedValue": "base64data",
          "createdAt": 1577836800,
          "updatedAt": 1590969600
        }
        "###);
    }
}
