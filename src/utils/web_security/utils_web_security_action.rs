use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{ClientUserShare, SharedResource, User},
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH, ContentSecurityPolicy,
        ContentSecurityPolicyDirective, ContentSecurityPolicyImportType,
        ContentSecurityPolicySource, UtilsWebSecurityActionResult,
    },
};
use anyhow::{anyhow, bail};
use serde::Deserialize;

#[allow(clippy::enum_variant_names)]
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityAction {
    #[serde(rename_all = "camelCase")]
    GetContentSecurityPolicy { policy_name: String },
    #[serde(rename_all = "camelCase")]
    SaveContentSecurityPolicy { policy: ContentSecurityPolicy },
    #[serde(rename_all = "camelCase")]
    ImportContentSecurityPolicy {
        policy_name: String,
        import_type: ContentSecurityPolicyImportType,
    },
    #[serde(rename_all = "camelCase")]
    RemoveContentSecurityPolicy { policy_name: String },
    #[serde(rename_all = "camelCase")]
    ShareContentSecurityPolicy { policy_name: String },
    #[serde(rename_all = "camelCase")]
    UnshareContentSecurityPolicy { policy_name: String },
    #[serde(rename_all = "camelCase")]
    SerializeContentSecurityPolicy {
        policy_name: String,
        source: ContentSecurityPolicySource,
    },
}

impl UtilsWebSecurityAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub async fn validate<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<()> {
        let assert_policy_name = |name: &str| -> Result<(), SecutilsError> {
            if name.is_empty() {
                return Err(SecutilsError::client("Policy name cannot be empty."));
            }

            if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                return Err(SecutilsError::client(format!(
                    "Policy name cannot be longer than {} characters.",
                    MAX_UTILS_ENTITY_NAME_LENGTH
                )));
            }

            Ok(())
        };

        match self {
            UtilsWebSecurityAction::SerializeContentSecurityPolicy { policy_name, .. }
            | UtilsWebSecurityAction::GetContentSecurityPolicy { policy_name }
            | UtilsWebSecurityAction::RemoveContentSecurityPolicy { policy_name }
            | UtilsWebSecurityAction::ShareContentSecurityPolicy { policy_name }
            | UtilsWebSecurityAction::UnshareContentSecurityPolicy { policy_name } => {
                assert_policy_name(policy_name)?;
            }
            UtilsWebSecurityAction::SaveContentSecurityPolicy { policy } => {
                if !policy.is_valid() {
                    bail!(SecutilsError::client("Policy is not valid."));
                }
            }
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name,
                import_type: source,
            } => {
                assert_policy_name(policy_name)?;

                match source {
                    ContentSecurityPolicyImportType::Text { text } => {
                        if text.is_empty() {
                            bail!(SecutilsError::client(
                                "Content security policy text to import source text cannot be empty."
                            ));
                        }
                    }
                    ContentSecurityPolicyImportType::Url { url, .. } => {
                        if !api.network.is_public_web_url(url).await {
                            bail!(
                                SecutilsError::client(format!("URL must be either `http` or `https` and have a valid public reachable domain name: {url}."))
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsWebSecurityActionResult> {
        let web_security = api.web_security();
        match self {
            UtilsWebSecurityAction::GetContentSecurityPolicy { policy_name } => {
                let users = api.users();
                Ok(UtilsWebSecurityActionResult::get(
                    web_security
                        .get_content_security_policy(user.id, &policy_name)
                        .await?,
                    users
                        .get_user_share_by_resource(
                            user.id,
                            &SharedResource::content_security_policy(policy_name),
                        )
                        .await?
                        .map(ClientUserShare::from),
                ))
            }
            UtilsWebSecurityAction::SaveContentSecurityPolicy { policy } => web_security
                .upsert_content_security_policy(user.id, policy)
                .await
                .map(|_| UtilsWebSecurityActionResult::save()),
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name,
                import_type,
            } => web_security
                .import_content_security_policy(user.id, policy_name, import_type)
                .await
                .map(|_| UtilsWebSecurityActionResult::import()),
            UtilsWebSecurityAction::RemoveContentSecurityPolicy { policy_name } => web_security
                .remove_content_security_policy(user.id, &policy_name)
                .await
                .map(|_| UtilsWebSecurityActionResult::remove()),
            UtilsWebSecurityAction::ShareContentSecurityPolicy { policy_name } => web_security
                .share_content_security_policy(user.id, &policy_name)
                .await
                .map(|user_share| {
                    UtilsWebSecurityActionResult::share(ClientUserShare::from(user_share))
                }),
            UtilsWebSecurityAction::UnshareContentSecurityPolicy { policy_name } => web_security
                .unshare_content_security_policy(user.id, &policy_name)
                .await
                .map(|user_share| user_share.map(ClientUserShare::from))
                .map(UtilsWebSecurityActionResult::unshare),
            UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name,
                source,
            } => {
                let policy = web_security
                    .get_content_security_policy(user.id, &policy_name)
                    .await?
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find user ({}) content security policy with the following name: {}", 
                            *user.id,
                            policy_name
                        )
                    })?;

                Ok(UtilsWebSecurityActionResult::serialize(
                    serialize_directives(
                        policy
                            .directives
                            .into_iter()
                            .filter(|directive| directive.is_supported_for_source(source)),
                    )?,
                    source,
                ))
            }
        }
    }
}

fn serialize_directives(
    directives: impl Iterator<Item = ContentSecurityPolicyDirective>,
) -> anyhow::Result<String> {
    let mut serialized_directives = vec![];
    for directive in directives {
        serialized_directives.push(String::try_from(directive)?);
    }

    Ok(serialized_directives.join("; "))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api, mock_api_with_network, mock_network_with_records, mock_user},
        utils::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicyImportType,
            ContentSecurityPolicySource, UtilsWebSecurityAction, UtilsWebSecurityActionResult,
        },
    };
    use std::{collections::HashSet, net::Ipv4Addr};
    use trust_dns_resolver::{
        proto::rr::{rdata::A, RData, Record},
        Name,
    };
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
{
    "type": "getContentSecurityPolicy",
    "value": { "policyName": "policy" }
}
          "#
            )?,
            UtilsWebSecurityAction::GetContentSecurityPolicy {
                policy_name: "policy".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "saveContentSecurityPolicy",
            "value": { "policy": { "n": "policy", "d": [{"n": "child-src", "v": ["'self'", "https://*"]}] } }
        }
                  "#
            )?,
            UtilsWebSecurityAction::SaveContentSecurityPolicy {
                policy: ContentSecurityPolicy {
                    name: "policy".to_string(),
                    directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string(), "https://*".to_string()]
                            .into_iter()
                            .collect()
                    )]
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "importContentSecurityPolicy",
            "value": { "policyName": "policy", "importType": { "type": "text", "text": "default-src 'self' https:" } }
        }
                  "#
            )?,
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name: "policy".to_string(),
                import_type: ContentSecurityPolicyImportType::Text {
                    text: "default-src 'self' https:".to_string()
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "removeContentSecurityPolicy",
            "value": { "policyName": "policy" }
        }
                  "#
            )?,
            UtilsWebSecurityAction::RemoveContentSecurityPolicy {
                policy_name: "policy".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "shareContentSecurityPolicy",
            "value": { "policyName": "policy" }
        }
                  "#
            )?,
            UtilsWebSecurityAction::ShareContentSecurityPolicy {
                policy_name: "policy".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "unshareContentSecurityPolicy",
            "value": { "policyName": "policy" }
        }
                  "#
            )?,
            UtilsWebSecurityAction::UnshareContentSecurityPolicy {
                policy_name: "policy".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebSecurityAction>(
                r#"
        {
            "type": "serializeContentSecurityPolicy",
            "value": { "policyName": "policy", "source": "meta" }
        }
                  "#
            )?,
            UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "policy".to_string(),
                source: ContentSecurityPolicySource::Meta,
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn validation() -> anyhow::Result<()> {
        let get_actions = |policy_name: String| {
            vec![
                UtilsWebSecurityAction::GetContentSecurityPolicy {
                    policy_name: policy_name.clone(),
                },
                UtilsWebSecurityAction::RemoveContentSecurityPolicy {
                    policy_name: policy_name.clone(),
                },
                UtilsWebSecurityAction::ShareContentSecurityPolicy {
                    policy_name: policy_name.clone(),
                },
                UtilsWebSecurityAction::UnshareContentSecurityPolicy {
                    policy_name: policy_name.clone(),
                },
                UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                    policy_name,
                    source: ContentSecurityPolicySource::Meta,
                },
            ]
        };

        let api = mock_api().await?;
        for action in get_actions("a".repeat(100)) {
            assert!(action.validate(&api).await.is_ok());
        }

        for action in get_actions("".to_string()) {
            assert_eq!(
                action.validate(&api).await.map_err(|err| err.to_string()),
                Err("Policy name cannot be empty.".to_string())
            );
        }

        for action in get_actions("a".repeat(101)) {
            assert_eq!(
                action.validate(&api).await.map_err(|err| err.to_string()),
                Err("Policy name cannot be longer than 100 characters.".to_string())
            );
        }

        assert!(UtilsWebSecurityAction::SaveContentSecurityPolicy {
            policy: ContentSecurityPolicy {
                name: "policy".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect()
                )]
            }
        }
        .validate(&api)
        .await
        .is_ok());

        assert_eq!(
            UtilsWebSecurityAction::SaveContentSecurityPolicy {
                policy: ContentSecurityPolicy {
                    name: "".to_string(),
                    directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect()
                    )]
                }
            }
            .validate(&api)
            .await
            .map_err(|err| err.to_string()),
            Err("Policy is not valid.".to_string())
        );

        assert_eq!(
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name: "".to_string(),
                import_type: ContentSecurityPolicyImportType::Text {
                    text: "default-src 'self' https:".to_string()
                }
            }
            .validate(&api)
            .await
            .map_err(|err| err.to_string()),
            Err("Policy name cannot be empty.".to_string())
        );

        assert_eq!(
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name: "a".repeat(101),
                import_type: ContentSecurityPolicyImportType::Text {
                    text: "default-src 'self' https:".to_string()
                }
            }
            .validate(&api)
            .await
            .map_err(|err| err.to_string()),
            Err("Policy name cannot be longer than 100 characters.".to_string())
        );

        assert_eq!(
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name: "policy".to_string(),
                import_type: ContentSecurityPolicyImportType::Text {
                    text: "".to_string()
                }
            }
            .validate(&api)
            .await
            .map_err(|err| err.to_string()),
            Err("Content security policy text to import source text cannot be empty.".to_string())
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;
        let valid_import_type = ContentSecurityPolicyImportType::Url {
            url: Url::parse("https://secutils.dev/my-page")?,
            source: ContentSecurityPolicySource::Meta,
            follow_redirects: true,
        };
        assert_eq!(
            UtilsWebSecurityAction::ImportContentSecurityPolicy {
                policy_name: "policy".to_string(),
                import_type: valid_import_type.clone()
            }
            .validate(&api_with_local_network)
            .await
            .map_err(|err| err.to_string()),
            Err("URL must be either `http` or `https` and have a valid public reachable domain name: https://secutils.dev/my-page.".to_string())
        );

        assert!(UtilsWebSecurityAction::ImportContentSecurityPolicy {
            policy_name: "policy".to_string(),
            import_type: valid_import_type
        }
        .validate(&api)
        .await
        .is_ok());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_retrieve_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        api.web_security()
            .upsert_content_security_policy(mock_user.id, policy.clone())
            .await?;

        let action = UtilsWebSecurityAction::GetContentSecurityPolicy {
            policy_name: policy.name.clone(),
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::get(Some(policy.clone()), None)
        );

        let policy_share = api
            .web_security()
            .share_content_security_policy(mock_user.id, &policy.name)
            .await?;

        let action = UtilsWebSecurityAction::GetContentSecurityPolicy {
            policy_name: policy.name.clone(),
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::get(Some(policy.clone()), Some(policy_share.into()))
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        api.web_security()
            .upsert_content_security_policy(mock_user.id, policy.clone())
            .await?;

        let action = UtilsWebSecurityAction::RemoveContentSecurityPolicy {
            policy_name: policy.name.clone(),
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::remove()
        );

        assert!(api
            .web_security()
            .get_content_security_policy(mock_user.id, &policy.name)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_share_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        api.web_security()
            .upsert_content_security_policy(mock_user.id, policy.clone())
            .await?;

        let action = UtilsWebSecurityAction::ShareContentSecurityPolicy {
            policy_name: policy.name.clone(),
        };
        let result = action.handle(mock_user.clone(), &api).await?;

        let policy_share = api
            .web_security()
            .share_content_security_policy(mock_user.id, &policy.name)
            .await?;
        assert_eq!(
            result,
            UtilsWebSecurityActionResult::share(policy_share.into())
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_unshare_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        api.web_security()
            .upsert_content_security_policy(mock_user.id, policy.clone())
            .await?;

        let policy_share = api
            .web_security()
            .share_content_security_policy(mock_user.id, &policy.name)
            .await?;

        let action = UtilsWebSecurityAction::UnshareContentSecurityPolicy {
            policy_name: policy.name.clone(),
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::unshare(Some(policy_share.clone().into()))
        );
        assert!(api
            .users()
            .get_user_share_by_resource(mock_user.id, &policy_share.resource)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_save_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        assert!(api
            .web_security()
            .get_content_security_policy(mock_user.id, &policy.name)
            .await?
            .is_none());

        let action = UtilsWebSecurityAction::SaveContentSecurityPolicy {
            policy: policy.clone(),
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::save()
        );
        assert_eq!(
            api.web_security()
                .get_content_security_policy(mock_user.id, &policy.name)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_serialize_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![
                ContentSecurityPolicyDirective::DefaultSrc(
                    ["'self'".to_string(), "https:".to_string()]
                        .into_iter()
                        .collect(),
                ),
                ContentSecurityPolicyDirective::Sandbox(HashSet::new()),
                ContentSecurityPolicyDirective::ReportTo(["prod-csp".to_string()]),
            ],
        };
        api.web_security()
            .upsert_content_security_policy(mock_user.id, policy.clone())
            .await?;

        let action = UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: policy.name.clone(),
            source: ContentSecurityPolicySource::EnforcingHeader,
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::serialize(
                "default-src 'self' https:; sandbox; report-to prod-csp".to_string(),
                ContentSecurityPolicySource::EnforcingHeader
            )
        );

        let action = UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: policy.name.clone(),
            source: ContentSecurityPolicySource::Meta,
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::serialize(
                "default-src 'self' https:".to_string(),
                ContentSecurityPolicySource::Meta
            )
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_import_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        assert!(api
            .web_security()
            .get_content_security_policy(mock_user.id, "policy-one")
            .await?
            .is_none());

        let action = UtilsWebSecurityAction::ImportContentSecurityPolicy {
            policy_name: "policy-one".to_string(),
            import_type: ContentSecurityPolicyImportType::Text {
                text: "default-src https:".to_string(),
            },
        };
        assert_eq!(
            action.handle(mock_user.clone(), &api).await?,
            UtilsWebSecurityActionResult::import()
        );
        assert_eq!(
            api.web_security()
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::DefaultSrc(
                    ["https:".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }
}
