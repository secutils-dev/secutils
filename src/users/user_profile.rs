use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Deserialize, Serialize, Debug, Eq, PartialEq, Clone)]
pub struct UserProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BTreeMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use crate::users::UserProfile;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(UserProfile::default(), @r###"{}"###);
            assert_json_snapshot!(UserProfile { data: Some([("KEY_1".to_string(), "VALUE_1".to_string())].into_iter().collect()) }, @r###"
            {
              "data": {
                "KEY_1": "VALUE_1"
              }
            }
            "###);
        });

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserProfile>("{}")?,
            UserProfile::default()
        );

        assert_eq!(
            serde_json::from_str::<UserProfile>(
                r###"
                {
                  "data": {
                    "KEY_1": "VALUE_1",
                    "KEY_2": "VALUE_2"
                  }
                }
                "###
            )?,
            UserProfile {
                data: Some(
                    [
                        ("KEY_1".to_string(), "VALUE_1".to_string()),
                        ("KEY_2".to_string(), "VALUE_2".to_string())
                    ]
                    .into_iter()
                    .collect()
                )
            }
        );

        Ok(())
    }
}
