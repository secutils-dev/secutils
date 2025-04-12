use crate::utils::{
    UtilsResource,
    web_scraping::{WebPageTracker, WebPageTrackerKind, WebPageTrackerTag},
    webhooks::Responder,
};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use uuid::Uuid;

/// Represents a context for the utility resource used for the structured logging.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UtilsResourceLogContext<'n> {
    /// Type of the utility resource.
    pub resource: UtilsResource,
    /// Unique id of the utility resource.
    pub resource_id: Uuid,
    /// Name of the utility resource.
    pub resource_name: &'n str,
}

impl Responder {
    /// Returns context used for the structured logging.
    pub fn log_context(&self) -> UtilsResourceLogContext {
        UtilsResourceLogContext {
            resource: UtilsResource::WebhooksResponders,
            resource_id: self.id,
            resource_name: self.name.as_str(),
        }
    }
}

impl<Tag: WebPageTrackerTag> WebPageTracker<Tag> {
    /// Returns context used for the structured logging.
    pub fn log_context(&self) -> UtilsResourceLogContext {
        UtilsResourceLogContext {
            resource: match Tag::KIND {
                WebPageTrackerKind::WebPageResources => UtilsResource::WebScrapingResources,
                WebPageTrackerKind::WebPageContent => UtilsResource::WebScrapingContent,
            },
            resource_id: self.id,
            resource_name: self.name.as_str(),
        }
    }
}

impl Serialize for UtilsResourceLogContext<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UtilsResourceLogContext", 3)?;
        state.serialize_field("resource", &Into::<(&str, &str)>::into(self.resource))?;
        state.serialize_field("resource_id", &self.resource_id.as_hyphenated().to_string())?;
        state.serialize_field("resource_name", self.resource_name)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        logging::UtilsResourceLogContext,
        tests::{MockResponderBuilder, MockWebPageTrackerBuilder},
        utils::{
            UtilsResource,
            web_scraping::{WebPageContentTrackerTag, WebPageResourcesTrackerTag},
        },
    };
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::CertificatesTemplates,
            resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
            resource_name: "my-cert-template",
        }, @r###"
        {
          "resource": [
            "certificates",
            "templates"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000001",
          "resource_name": "my-cert-template"
        }
        "###);
        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::WebhooksResponders,
            resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
            resource_name: "my-responder",
        }, @r###"
        {
          "resource": [
            "webhooks",
            "responders"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000002",
          "resource_name": "my-responder"
        }
        "###);

        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::WebScrapingResources,
            resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
            resource_name: "my-tracker",
        }, @r###"
        {
          "resource": [
            "web_scraping",
            "resources"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000002",
          "resource_name": "my-tracker"
        }
        "###);

        assert_json_snapshot!(UtilsResourceLogContext {
            resource: UtilsResource::WebScrapingContent,
            resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
            resource_name: "my-tracker",
        }, @r###"
        {
          "resource": [
            "web_scraping",
            "content"
          ],
          "resource_id": "00000000-0000-0000-0000-000000000002",
          "resource_name": "my-tracker"
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
                resource_name: "some-name"
            }
        );

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        assert_eq!(
            tracker.log_context(),
            UtilsResourceLogContext {
                resource: UtilsResource::WebScrapingResources,
                resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
                resource_name: "some-name"
            }
        );

        let tracker = MockWebPageTrackerBuilder::<WebPageContentTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        assert_eq!(
            tracker.log_context(),
            UtilsResourceLogContext {
                resource: UtilsResource::WebScrapingContent,
                resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
                resource_name: "some-name"
            }
        );

        Ok(())
    }
}
