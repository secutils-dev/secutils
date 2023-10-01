use serde::{Deserialize, Serialize};

/// Describes a resource that can be shared with other users.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SharedResource {
    ContentSecurityPolicy { policy_name: String },
}

impl SharedResource {
    /// Creates a new shared resource referencing a user content security policy.
    pub fn content_security_policy<T: Into<String>>(policy_name: T) -> SharedResource {
        SharedResource::ContentSecurityPolicy {
            policy_name: policy_name.into(),
        }
    }
}

/// A special version of SharedResource that can be safely serialized for the client side since not
/// all Serde attributes we need can be serialized with postcard (main serialization format).
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum ClientSharedResource {
    #[serde(rename_all = "camelCase")]
    ContentSecurityPolicy { policy_name: String },
}

impl From<SharedResource> for ClientSharedResource {
    fn from(value: SharedResource) -> Self {
        match value {
            SharedResource::ContentSecurityPolicy { policy_name } => {
                Self::ContentSecurityPolicy { policy_name }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientSharedResource, SharedResource};

    #[test]
    fn can_create_csp_shared_resource() {
        assert_eq!(
            SharedResource::content_security_policy("my-policy"),
            SharedResource::ContentSecurityPolicy {
                policy_name: "my-policy".to_string()
            }
        );
    }

    #[test]
    fn can_create_client_shared_resource() {
        assert_eq!(
            ClientSharedResource::from(SharedResource::content_security_policy("my-policy")),
            ClientSharedResource::ContentSecurityPolicy {
                policy_name: "my-policy".to_string()
            }
        );
    }
}
