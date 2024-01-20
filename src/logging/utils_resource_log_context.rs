use crate::utils::{webhooks::Responder, UtilsResource};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use uuid::Uuid;

/// Represents a context for the utility resource used for the structured logging.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UtilsResourceLogContext {
    /// Type of the utility resource.
    pub resource: UtilsResource,
    /// Unique id of the utility resource.
    pub resource_id: Uuid,
}

impl Responder {
    /// Returns context used for the structured logging.
    pub fn log_context(&self) -> UtilsResourceLogContext {
        UtilsResourceLogContext {
            resource: UtilsResource::WebhooksResponders,
            resource_id: self.id,
        }
    }
}

impl Serialize for UtilsResourceLogContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UtilsResourceLogContext", 2)?;
        state.serialize_field("resource", &Into::<(&str, &str)>::into(self.resource))?;
        state.serialize_field("resource_id", &self.resource_id.as_hyphenated().to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{MockResponderBuilder, UtilsResourceLogContext},
        utils::UtilsResource,
    };
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::CertificatesTemplates,
            resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
        }, @r###"
        {
          "resource": [
            "certificates",
            "templates"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000001"
        }
        "###);
        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::WebhooksResponders,
            resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
        }, @r###"
        {
          "resource": [
            "webhooks",
            "responders"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000002"
        }
        "###);

        Ok(())
    }

    #[test]
    fn log_context() -> anyhow::Result<()> {
        let responder = MockResponderBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "/",
        )?
        .build();

        assert_eq!(
            responder.log_context(),
            UtilsResourceLogContext {
                resource: UtilsResource::WebhooksResponders,
                resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
            }
        );

        Ok(())
    }
}
