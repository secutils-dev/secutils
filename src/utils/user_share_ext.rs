use crate::{
    users::{SharedResource, UserShare},
    utils::{UtilsAction, UtilsResource, UtilsResourceOperation},
};

impl UserShare {
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
                            resource_id: Some(resource_id),
                            operation,
                        },
                    ) => {
                        template_id == resource_id
                            && operation == &UtilsResourceOperation::CertificatesTemplateGenerate
                    }
                    _ => false,
                }
            }
            SharedResource::ContentSecurityPolicy { policy_id } => {
                match (resource, action) {
                    // Any user can access content security policy and serialize it.
                    (
                        UtilsResource::WebSecurityContentSecurityPolicies,
                        UtilsAction::Get { resource_id },
                    ) => policy_id == resource_id,
                    (
                        UtilsResource::WebSecurityContentSecurityPolicies,
                        UtilsAction::Execute {
                            resource_id: Some(resource_id),
                            operation,
                        },
                    ) => policy_id == resource_id
                        && operation
                            == &UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize,
                    _ => false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::{SharedResource, UserId, UserShare},
        utils::{UtilsAction, UtilsResource, UtilsResourceOperation},
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn properly_checks_action_authorization_for_shared_csp() {
        let policy_id = uuid!("00000000-0000-0000-0000-000000000001");
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::new(),
            resource: SharedResource::content_security_policy(policy_id),
            created_at: OffsetDateTime::now_utc(),
        };

        let unauthorized_actions = vec![
            UtilsAction::List,
            UtilsAction::Get {
                resource_id: uuid!("00000000-0000-0000-0000-000000000002"),
            },
            UtilsAction::Create,
            UtilsAction::Update {
                resource_id: policy_id,
            },
            UtilsAction::Delete {
                resource_id: policy_id,
            },
            UtilsAction::Share {
                resource_id: policy_id,
            },
            UtilsAction::Unshare {
                resource_id: policy_id,
            },
            UtilsAction::Execute {
                resource_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                operation: UtilsResourceOperation::CertificatesTemplateGenerate,
            },
        ];
        for action in unauthorized_actions {
            assert!(!user_share
                .is_action_authorized(&action, &UtilsResource::WebSecurityContentSecurityPolicies));
        }

        let authorized_actions = vec![
            UtilsAction::Get {
                resource_id: policy_id,
            },
            UtilsAction::Execute {
                resource_id: Some(policy_id),
                operation: UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize,
            },
        ];
        for action in authorized_actions {
            assert!(user_share
                .is_action_authorized(&action, &UtilsResource::WebSecurityContentSecurityPolicies));
        }
    }

    #[test]
    fn properly_checks_action_authorization_for_shared_certificate_template() -> anyhow::Result<()>
    {
        let template_id = uuid!("00000000-0000-0000-0000-000000000001");
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::new(),
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
            UtilsAction::Share {
                resource_id: template_id,
            },
            UtilsAction::Unshare {
                resource_id: template_id,
            },
            UtilsAction::Execute {
                resource_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
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
                resource_id: Some(template_id),
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
