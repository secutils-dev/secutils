use serde_derive::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct Util {
    #[serde(skip_serializing)]
    pub id: i64,
    pub handle: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub keywords: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utils: Option<Vec<Util>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::Util;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let util_without_optional = Util {
            id: 1,
            handle: "some-handle".to_string(),
            name: "some-name".to_string(),
            keywords: "some keywords".to_string(),
            utils: None,
        };
        assert_json_snapshot!(util_without_optional, @r###"
        {
          "handle": "some-handle",
          "name": "some-name"
        }
        "###);

        let util_with_optional = Util {
            id: 1,
            handle: "some-handle".to_string(),
            name: "some-name".to_string(),
            keywords: "some keywords".to_string(),
            utils: Some(vec![util_without_optional]),
        };
        assert_json_snapshot!(util_with_optional, @r###"
        {
          "handle": "some-handle",
          "name": "some-name",
          "utils": [
            {
              "handle": "some-handle",
              "name": "some-name"
            }
          ]
        }
        "###);

        Ok(())
    }
}
