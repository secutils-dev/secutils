use serde::{Deserialize, Serialize};

/// Represents type of the web page tracker (e.g. resources, content, etc.).
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Hash, Eq)]
pub enum WebPageTrackerKind {
    WebPageResources,
    WebPageContent,
}

impl TryFrom<WebPageTrackerKind> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: WebPageTrackerKind) -> Result<Self, Self::Error> {
        Ok(postcard::to_stdvec(&value)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::WebPageTrackerKind;
    use insta::assert_debug_snapshot;

    #[test]
    fn serialize() -> anyhow::Result<()> {
        assert_eq!(
            Vec::try_from(WebPageTrackerKind::WebPageResources)?,
            vec![0]
        );
        assert_eq!(Vec::try_from(WebPageTrackerKind::WebPageContent)?, vec![1]);

        Ok(())
    }

    #[test]
    fn deserialize() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<WebPageTrackerKind>([0].as_ref())?,
            WebPageTrackerKind::WebPageResources
        );

        assert_eq!(
            postcard::from_bytes::<WebPageTrackerKind>([1].as_ref())?,
            WebPageTrackerKind::WebPageContent
        );

        assert_debug_snapshot!(postcard::from_bytes::<WebPageTrackerKind>([2].as_ref()), @r###"
        Err(
            SerdeDeCustom,
        )
        "###);

        Ok(())
    }
}
