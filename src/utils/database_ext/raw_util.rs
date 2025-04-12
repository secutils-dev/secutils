use crate::utils::Util;
use anyhow::bail;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUtil {
    pub id: i32,
    pub handle: String,
    pub name: String,
    pub keywords: Option<String>,
    pub parent_id: Option<i32>,
}

impl TryFrom<RawUtil> for Util {
    type Error = anyhow::Error;

    fn try_from(raw_util: RawUtil) -> Result<Self, Self::Error> {
        if raw_util.id <= 0 || raw_util.handle.is_empty() || raw_util.name.is_empty() {
            bail!("Malformed raw utility: {:?}", raw_util);
        }

        if let Some(parent_id) = raw_util.parent_id {
            if parent_id <= 0 {
                bail!("Invalid raw utility parent id: {}", parent_id);
            }
        }

        Ok(Util {
            id: raw_util.id,
            handle: raw_util.handle,
            name: raw_util.name,
            keywords: raw_util.keywords,
            utils: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUtil;
    use crate::utils::Util;
    use insta::assert_debug_snapshot;

    #[test]
    fn can_convert_into_user_without_optional_fields() -> anyhow::Result<()> {
        assert_debug_snapshot!(
            Util::try_from(RawUtil {
                id: 1,
                handle: "some-handle".to_string(),
                name: "some-name".to_string(),
                keywords: None,
                parent_id: None,
            })?,
            @r###"
        Util {
            id: 1,
            handle: "some-handle",
            name: "some-name",
            keywords: None,
            utils: None,
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_user_with_optional_fields() -> anyhow::Result<()> {
        assert_debug_snapshot!(
            Util::try_from(RawUtil {
                id: 1,
                handle: "some-handle".to_string(),
                name: "some-name".to_string(),
                keywords: Some("some-keywords".to_string()),
                parent_id: Some(2),
            })?,
            @r###"
        Util {
            id: 1,
            handle: "some-handle",
            name: "some-name",
            keywords: Some(
                "some-keywords",
            ),
            utils: None,
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(
            Util::try_from(RawUtil {
                id: 0,
                handle: "some-handle".to_string(),
                name: "some-name".to_string(),
                keywords: Some("some-keywords".to_string()),
                parent_id: None,
            })
            .is_err()
        );

        assert!(
            Util::try_from(RawUtil {
                id: 1,
                handle: "".to_string(),
                name: "some-name".to_string(),
                keywords: Some("some-keywords".to_string()),
                parent_id: None,
            })
            .is_err()
        );

        assert!(
            Util::try_from(RawUtil {
                id: 1,
                handle: "some-handle".to_string(),
                name: "".to_string(),
                keywords: Some("some-keywords".to_string()),
                parent_id: None,
            })
            .is_err()
        );

        assert!(
            Util::try_from(RawUtil {
                id: 1,
                handle: "some-handle".to_string(),
                name: "some-name".to_string(),
                keywords: Some("some-keywords".to_string()),
                parent_id: Some(0),
            })
            .is_err()
        );

        Ok(())
    }
}
