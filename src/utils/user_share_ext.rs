use crate::{
    users::{SharedResource, UserShare},
    utils::{
        UtilsAction, UtilsLegacyAction, UtilsResource, UtilsResourceOperation,
        UtilsWebSecurityAction,
    },
};

impl UserShare {
    /// Checks if the user share is authorized to perform the specified action.
    pub fn is_legacy_action_authorized(&self, action: &UtilsLegacyAction) -> bool {
        match (&self.resource, action) {
            // Any user can access and serialize content of the shared content security policy.
            (
                SharedResource::ContentSecurityPolicy {
                    policy_name: resource_policy_name,
                },
                UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                    policy_name,
                })
                | UtilsLegacyAction::WebSecurity(
                    UtilsWebSecurityAction::SerializeContentSecurityPolicy { policy_name, .. },
                ),
            ) if resource_policy_name == policy_name => true,
            _ => false,
        }
    }

    /// Checks if the user share is authorized to perform the specified action.
    pub fn is_action_authorized(&self, action: &UtilsAction, resource: &UtilsResource) -> bool {
        match &self.resource {
            SharedResource::CertificateTemplate { template_id } => {
                match (resource, action) {
                    // Any user can access certificate template and generate certificate/key pair.
                    (UtilsResource::CertificatesTemplates, UtilsAction::Get { resource_id }) => {
                        template_id == resource_id
                    }
                    (
                        UtilsResource::CertificatesTemplates,
                        UtilsAction::Execute {
                            resource_id,
                            operation,
                        },
                    ) => {
                        template_id == resource_id
                            && operation == &UtilsResourceOperation::CertificatesTemplateGenerate
                    }
                    _ => false,
                }
            }
            SharedResource::ContentSecurityPolicy { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::{SharedResource, UserId, UserShare},
        utils::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicySource,
            UtilsAction, UtilsLegacyAction, UtilsResource, UtilsResourceOperation,
            UtilsWebSecurityAction,
        },
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn properly_checks_action_authorization_for_shared_csp() {
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::empty(),
            resource: SharedResource::content_security_policy("my-policy"),
            created_at: OffsetDateTime::now_utc(),
        };

        let unauthorized_actions = vec![
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::SaveContentSecurityPolicy {
                policy: ContentSecurityPolicy {
                    name: "".to_string(),
                    directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    )],
                },
            }),
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::RemoveContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::ShareContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::UnshareContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                policy_name: "not-my-policy".to_string(),
            }),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "not-my-policy".to_string(),
                    source: ContentSecurityPolicySource::Meta,
                },
            ),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "not-my-policy".to_string(),
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            ),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "not-my-policy".to_string(),
                    source: ContentSecurityPolicySource::ReportOnlyHeader,
                },
            ),
        ];
        for action in unauthorized_actions {
            assert!(!user_share.is_legacy_action_authorized(&action));
        }

        let authorized_actions = vec![
            UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::GetContentSecurityPolicy {
                policy_name: "my-policy".to_string(),
            }),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "my-policy".to_string(),
                    source: ContentSecurityPolicySource::Meta,
                },
            ),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "my-policy".to_string(),
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            ),
            UtilsLegacyAction::WebSecurity(
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name: "my-policy".to_string(),
                    source: ContentSecurityPolicySource::ReportOnlyHeader,
                },
            ),
        ];
        for action in authorized_actions {
            assert!(user_share.is_legacy_action_authorized(&action));
        }
    }

    #[test]
    fn properly_checks_action_authorization_for_shared_certificate_template() -> anyhow::Result<()>
    {
        let template_id = uuid!("00000000-0000-0000-0000-000000000001");
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::empty(),
            resource: SharedResource::certificate_template(template_id),
            created_at: OffsetDateTime::now_utc(),
        };

        let unauthorized_actions = vec![
            UtilsAction::List,
            UtilsAction::Get {
                resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
            },
            UtilsAction::Create,
            UtilsAction::Update {
                resource_id: template_id,
            },
            UtilsAction::Delete {
                resource_id: template_id,
            },
            UtilsAction::Execute {
                resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
                operation: UtilsResourceOperation::CertificatesTemplateGenerate,
            },
        ];
        for action in unauthorized_actions {
            assert!(
                !user_share.is_action_authorized(&action, &UtilsResource::CertificatesTemplates)
            );
        }

        let authorized_actions = vec![
            UtilsAction::Get {
                resource_id: template_id,
            },
            UtilsAction::Execute {
                resource_id: template_id,
                operation: UtilsResourceOperation::CertificatesTemplateGenerate,
            },
        ];
        for action in authorized_actions.iter() {
            assert!(user_share.is_action_authorized(action, &UtilsResource::CertificatesTemplates));
        }

        for action in authorized_actions {
            assert!(
                !user_share.is_action_authorized(&action, &UtilsResource::CertificatesPrivateKeys)
            );
        }

        Ok(())
    }
}
