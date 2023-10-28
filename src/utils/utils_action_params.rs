use crate::error::Error as SecutilsError;
use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Describes the parameters of an action.
pub struct UtilsActionParams(Value);
impl UtilsActionParams {
    /// Creates a new action parameters instance from the given JSON value.
    pub fn json(value: Value) -> Self {
        Self(value)
    }

    /// Consumes and returns the inner value deserialized to a specified type.
    pub fn into_inner<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        Ok(serde_json::from_value(self.0).map_err(|err| {
            SecutilsError::client_with_root_cause(
                anyhow!(err).context("Invalid action parameters."),
            )
        })?)
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsActionParams;
    use crate::error::Error as SecutilsError;
    use insta::assert_debug_snapshot;
    use serde_json::json;

    #[test]
    fn properly_returns_inner_value() {
        assert_eq!(
            UtilsActionParams::json(json!([1, 2, 3]))
                .into_inner::<Vec<u8>>()
                .unwrap(),
            vec![1, 2, 3]
        );

        assert_debug_snapshot!(UtilsActionParams::json(json!([1, 2, 3]))
                .into_inner::<Vec<String>>()
                .unwrap_err()
                .downcast::<SecutilsError>(), @r###"
        Ok(
            Error {
                context: "Invalid action parameters.",
                source: Error("invalid type: number, expected a string", line: 0, column: 0),
            },
        )
        "###);
    }
}
