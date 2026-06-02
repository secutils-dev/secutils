mod api_ext;
mod database_ext;
mod responders;

pub use self::{
    api_ext::{RespondersCreateParams, RespondersRequestCreateParams, RespondersUpdateParams},
    responders::{
        Responder, ResponderLocation, ResponderMethod, ResponderNotificationSettings,
        ResponderPathType, ResponderRequest, ResponderRequestHeaders, ResponderScriptContext,
        ResponderScriptResult, ResponderSettings, ResponderStats,
    },
};

#[cfg(test)]
pub mod tests {
    pub use crate::utils::webhooks::api_ext::{RespondersCreateParams, RespondersUpdateParams};
    use crate::{
        users::{EntityTag, SecretsAccess},
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
        },
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    pub struct MockResponderBuilder {
        responder: Responder,
    }

    impl MockResponderBuilder {
        pub fn create(id: Uuid, name: &str, path: &str) -> anyhow::Result<Self> {
            Ok(Self {
                responder: Responder {
                    id,
                    name: name.to_string(),
                    location: ResponderLocation {
                        path_type: ResponderPathType::Exact,
                        path: path.to_string(),
                        subdomain_prefix: None,
                    },
                    method: ResponderMethod::Any,
                    enabled: true,
                    settings: ResponderSettings {
                        requests_to_track: 0,
                        status_code: 200,
                        body: None,
                        headers: None,
                        script: None,
                        secrets: SecretsAccess::None,
                        notifications: None,
                    },
                    tags: vec![],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn with_method(mut self, method: ResponderMethod) -> Self {
            self.responder.method = method;
            self
        }

        pub fn with_body(mut self, body: &str) -> Self {
            self.responder.settings.body = Some(body.to_string());
            self
        }

        pub fn with_location(mut self, location: ResponderLocation) -> Self {
            self.responder.location = location;
            self
        }

        pub fn with_tag_ids(mut self, tag_ids: &[Uuid]) -> Self {
            self.responder.tags = tag_ids.iter().map(|id| EntityTag::from(*id)).collect();
            self
        }

        pub fn build(self) -> Responder {
            self.responder
        }
    }
}
