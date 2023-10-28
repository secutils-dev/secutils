use crate::error::Error as SecutilsError;
use anyhow::anyhow;
use serde::Serialize;
use serde_json::Value;

/// Describes the result of an action.
#[derive(Debug)]
pub struct UtilsActionResult(Option<Value>);
impl UtilsActionResult {
    /// Creates a new action result in JSON format from the given serializable value.
    pub fn json(value: impl Serialize) -> anyhow::Result<Self> {
        let json_value = serde_json::to_value(value).map_err(|err| {
            SecutilsError::client_with_root_cause(
                anyhow!(err).context("Unable to serialize action result."),
            )
        })?;
        Ok(Self(Some(json_value)))
    }

    /// Creates a new empty action result.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Consumes and returns the inner value.
    pub fn into_inner(self) -> Option<Value> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsActionResult;
    use insta::assert_debug_snapshot;
    use serde_json::json;

    #[test]
    fn properly_returns_inner_value() {
        assert_eq!(
            UtilsActionResult::json(json!([1, 2, 3]))
                .unwrap()
                .into_inner()
                .unwrap(),
            json!([1, 2, 3])
        );

        assert_eq!(UtilsActionResult::empty().into_inner(), None);
    }

    #[test]
    fn properly_serializes_to_json() {
        assert_debug_snapshot!(UtilsActionResult::json(json!([1, 2, 3])).unwrap(), @r###"
        UtilsActionResult(
            Some(
                Array [
                    Number(1),
                    Number(2),
                    Number(3),
                ],
            ),
        )
        "###);

        assert_debug_snapshot!(UtilsActionResult::empty(), @r###"
        UtilsActionResult(
            None,
        )
        "###);
    }
}
