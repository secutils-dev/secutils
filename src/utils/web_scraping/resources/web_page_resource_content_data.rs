use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebPageResourceContentData {
    /// Raw resource content.
    Raw(String),
    /// SHA-1 hash digest of the resource content.
    Sha1(String),
    /// Trend Micro locality sensitive hash digest of the resource content.
    Tlsh(String),
}

impl WebPageResourceContentData {
    /// Returns reference to the underlying data value.
    pub fn value(&self) -> &str {
        match self {
            WebPageResourceContentData::Raw(value) => value,
            WebPageResourceContentData::Sha1(value) => value,
            WebPageResourceContentData::Tlsh(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResourceContentData;
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourceContentData::Raw("Some content".to_string()), @r###"
        {
          "raw": "Some content"
        }
        "###);
        assert_json_snapshot!(WebPageResourceContentData::Tlsh("9590220E23308028".to_string()), @r###"
        {
          "tlsh": "9590220E23308028"
        }
        "###);
        assert_json_snapshot!(WebPageResourceContentData::Sha1("eeb57986d46355a4ccfab37c3071f40e2b14ab07".to_string()), @r###"
        {
          "sha1": "eeb57986d46355a4ccfab37c3071f40e2b14ab07"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourceContentData>(
                &json!({ "raw": "Some content" }).to_string()
            )?,
            WebPageResourceContentData::Raw("Some content".to_string())
        );

        assert_eq!(
            serde_json::from_str::<WebPageResourceContentData>(
                &json!({ "tlsh": "9590220E23308028" }).to_string()
            )?,
            WebPageResourceContentData::Tlsh("9590220E23308028".to_string())
        );

        assert_eq!(
            serde_json::from_str::<WebPageResourceContentData>(
                &json!({ "sha1": "eeb57986d46355a4ccfab37c3071f40e2b14ab07" }).to_string()
            )?,
            WebPageResourceContentData::Sha1(
                "eeb57986d46355a4ccfab37c3071f40e2b14ab07".to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn properly_returns_value() -> anyhow::Result<()> {
        assert_eq!(
            WebPageResourceContentData::Raw("Some content".to_string()).value(),
            "Some content"
        );
        assert_eq!(
            WebPageResourceContentData::Tlsh("9590220E23308028".to_string()).value(),
            "9590220E23308028"
        );
        assert_eq!(
            WebPageResourceContentData::Sha1(
                "eeb57986d46355a4ccfab37c3071f40e2b14ab07".to_string()
            )
            .value(),
            "eeb57986d46355a4ccfab37c3071f40e2b14ab07"
        );

        Ok(())
    }
}
