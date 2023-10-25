use crate::{
    users::{SharedResource, UserShare},
    utils::{UtilsAction, UtilsCertificatesAction, UtilsWebSecurityAction},
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
            // Any user can access certificate template and generate certificate/key pair.
            (
                SharedResource::CertificateTemplate {
                    template_id: resource_template_id,
                },
                UtilsAction::Certificates(UtilsCertificatesAction::GetCertificateTemplate {
                    template_id,
                })
                | UtilsAction::Certificates(UtilsCertificatesAction::GenerateSelfSignedCertificate {
                    template_id,
                    ..
                }),
            ) if resource_template_id == template_id => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockCertificateAttributes,
        users::{SharedResource, UserId, UserShare},
        utils::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicySource,
            ExportFormat, PrivateKeyAlgorithm, PrivateKeySize, SignatureAlgorithm, UtilsAction,
            UtilsCertificatesAction, UtilsWebSecurityAction, Version,
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
            UtilsAction::Certificates(UtilsCertificatesAction::GetCertificateTemplates),
            UtilsAction::Certificates(UtilsCertificatesAction::CreateCertificateTemplate {
                template_name: "a".to_string(),
                attributes: MockCertificateAttributes::new(
                    PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    },
                    SignatureAlgorithm::Sha256,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    Version::One,
                )
                .build(),
            }),
            UtilsAction::Certificates(UtilsCertificatesAction::UpdateCertificateTemplate {
                template_id,
                template_name: None,
                attributes: None,
            }),
            UtilsAction::Certificates(UtilsCertificatesAction::RemoveCertificateTemplate {
                template_id,
            }),
            UtilsAction::Certificates(UtilsCertificatesAction::ShareCertificateTemplate {
                template_id,
            }),
            UtilsAction::Certificates(UtilsCertificatesAction::UnshareCertificateTemplate {
                template_id,
            }),
        ];
        for action in unauthorized_actions {
            assert!(!user_share.is_action_authorized(&action));
        }

        let authorized_actions = vec![
            UtilsAction::Certificates(UtilsCertificatesAction::GetCertificateTemplate {
                template_id,
            }),
            UtilsAction::Certificates(UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_id,
                format: ExportFormat::Pem,
                passphrase: None,
            }),
        ];
        for action in authorized_actions {
            assert!(user_share.is_action_authorized(&action));
        }

        Ok(())
    }
}
