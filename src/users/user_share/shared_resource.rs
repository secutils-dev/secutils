use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Describes a resource that can be shared with other users.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SharedResource {
    ContentSecurityPolicy { policy_name: String },
    CertificateTemplate { template_id: Uuid },
}

impl SharedResource {
    /// Creates a new shared resource referencing a user content security policy.
    pub fn content_security_policy<T: Into<String>>(policy_name: T) -> SharedResource {
        SharedResource::ContentSecurityPolicy {
            policy_name: policy_name.into(),
        }
    }

    /// Creates a new shared resource referencing a user certificate template.
    pub fn certificate_template(template_id: Uuid) -> SharedResource {
        SharedResource::CertificateTemplate { template_id }
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
    #[serde(rename_all = "camelCase")]
    CertificateTemplate { template_id: Uuid },
}

impl From<SharedResource> for ClientSharedResource {
    fn from(value: SharedResource) -> Self {
        match value {
            SharedResource::ContentSecurityPolicy { policy_name } => {
                Self::ContentSecurityPolicy { policy_name }
            }
            SharedResource::CertificateTemplate { template_id } => {
                Self::CertificateTemplate { template_id }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientSharedResource, SharedResource};
    use uuid::uuid;

    #[test]
    fn can_create_csp_shared_resource() {
        assert_eq!(
            SharedResource::content_security_policy("my-policy"),
            SharedResource::ContentSecurityPolicy {
                policy_name: "my-policy".to_string()
            }
        );

        assert_eq!(
            SharedResource::certificate_template(uuid!("00000000-0000-0000-0000-000000000001")),
            SharedResource::CertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001")
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

        assert_eq!(
            ClientSharedResource::from(SharedResource::certificate_template(uuid!(
                "00000000-0000-0000-0000-000000000001"
            ))),
            ClientSharedResource::CertificateTemplate {
                template_id: uuid!("00000000-0000-0000-0000-000000000001")
            }
        );
    }
}
