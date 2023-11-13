use serde::Serialize;

/// Represents a web page resource diff status.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WebPageResourceDiffStatus {
    /// Indicates that the resource was added since last revision.
    Added,
    /// Indicates that the resource was removed since last revision.
    Removed,
    /// Indicates that the resource was changed since last revision.
    Changed,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageResourceDiffStatus;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageResourceDiffStatus::Added, @r###""added""###);
        assert_json_snapshot!(WebPageResourceDiffStatus::Removed, @r###""removed""###);
        assert_json_snapshot!(WebPageResourceDiffStatus::Changed, @r###""changed""###);

        Ok(())
    }
}
