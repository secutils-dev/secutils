use crate::{
    users::{SharedResource, UserShare},
    utils::{UtilsAction, UtilsWebSecurityAction},
};

impl UserShare {
    /// Checks if the user share is authorized to perform the specified action.
    pub fn is_action_authorized(&self, action: &UtilsAction) -> bool {
        match (&self.resource, action) {
            // Any user can access and serialize content of the shared content security policy.
            (
                SharedResource::ContentSecurityPolicy {
                    policy_name: resource_policy_name,
                },
                UtilsAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                    policy_name,
                })
                | UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name,
                    ..
                }),
            ) if resource_policy_name == policy_name => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::{SharedResource, UserId, UserShare},
        utils::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicySource,
            UtilsAction, UtilsWebSecurityAction,
        },
    };
    use time::OffsetDateTime;

    #[test]
    fn properly_checks_action_authorization_for_shared_csp() {
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::empty(),
            resource: SharedResource::content_security_policy("my-policy"),
            created_at: OffsetDateTime::now_utc(),
        };

        let unauthorized_actions = vec![
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SaveContentSecurityPolicy {
                policy: ContentSecurityPolicy {
                    name: "".to_string(),
                    directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    )],
                },
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::RemoveContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::ShareContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::UnshareContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
                source: ContentSecurityPolicySource::Meta,
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
                source: ContentSecurityPolicySource::EnforcingHeader,
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
                source: ContentSecurityPolicySource::ReportOnlyHeader,
            }),
        ];
        for action in unauthorized_actions {
            assert!(!user_share.is_action_authorized(&action));
        }

        let authorized_actions = vec![
            UtilsAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                policy_name: "my-policy".to_string(),
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "my-policy".to_string(),
                source: ContentSecurityPolicySource::Meta,
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "my-policy".to_string(),
                source: ContentSecurityPolicySource::EnforcingHeader,
            }),
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "my-policy".to_string(),
                source: ContentSecurityPolicySource::ReportOnlyHeader,
            }),
        ];
        for action in authorized_actions {
            assert!(user_share.is_action_authorized(&action));
        }
    }
}
