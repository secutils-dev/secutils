mod content_security_policies_create_params;
mod content_security_policies_serialize_params;
mod content_security_policies_update_params;
pub mod content_security_policy_content;
mod csp_meta_parser;

pub use self::{
    content_security_policies_create_params::ContentSecurityPoliciesCreateParams,
    content_security_policies_serialize_params::ContentSecurityPoliciesSerializeParams,
    content_security_policies_update_params::ContentSecurityPoliciesUpdateParams,
    content_security_policy_content::ContentSecurityPolicyContent,
};
use crate::{
    api::Api,
    config::SECUTILS_USER_AGENT,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::{SharedResource, UserId, UserShare},
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_security::api_ext::csp_meta_parser::CspMetaParser, ContentSecurityPolicy,
        ContentSecurityPolicyDirective, ContentSecurityPolicySource,
    },
};
use anyhow::{anyhow, bail};
use content_security_policy::{Policy, PolicyDisposition, PolicySource};
use reqwest::redirect::Policy as RedirectPolicy;
use time::OffsetDateTime;
use uuid::Uuid;

/// API extension to work with web security utilities.
pub struct WebSecurityApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> WebSecurityApiExt<'a, DR, ET> {
    /// Creates WebSecurity API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Returns content security policy by its ID.
    pub async fn get_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<ContentSecurityPolicy>> {
        self.api
            .db
            .web_security()
            .get_content_security_policy(user_id, id)
            .await
    }

    /// Retrieves all content security policies that belong to the specified user.
    pub async fn get_content_security_policies(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<ContentSecurityPolicy>> {
        self.api
            .db
            .web_security()
            .get_content_security_policies(user_id)
            .await
    }

    /// Creates content security policy with the specified parameters and stores it in the database.
    pub async fn create_content_security_policy(
        &self,
        user_id: UserId,
        params: ContentSecurityPoliciesCreateParams,
    ) -> anyhow::Result<ContentSecurityPolicy> {
        // First, fetch policy text if needed.
        let directives = match params.content {
            ContentSecurityPolicyContent::Directives(directives) => directives,
            ContentSecurityPolicyContent::Serialized(policy_text) => {
                let directives = Self::deserialize_directives(&policy_text);
                log::debug!(
                    "Deserialized {} content security policy directives: {policy_text}",
                    directives.len()
                );
                directives
            }
            ContentSecurityPolicyContent::Remote {
                url,
                follow_redirects,
                source,
            } => {
                if !self.api.network.is_public_web_url(&url).await {
                    bail!(SecutilsError::client(
                        format!("Remote URL must be either `http` or `https` and have a valid public reachable domain name, but received {url}.")
                    ));
                }

                let client = reqwest::ClientBuilder::new()
                    .redirect(if follow_redirects {
                        let host_url = url.host_str().map(|host| host.to_string());
                        RedirectPolicy::custom(move |attempt| {
                            if attempt.previous().len() > 10 {
                                log::error!("Too many redirects for host ({host_url:?}).");
                                attempt
                                    .error(format!("Too many redirects for host ({host_url:?})."))
                            } else if attempt.url().host_str() != host_url.as_deref() {
                                log::error!(
                                    "Redirected from host ({host_url:?}) to different host: {:?}.",
                                    attempt.url().host_str()
                                );
                                attempt.stop()
                            } else {
                                attempt.follow()
                            }
                        })
                    } else {
                        RedirectPolicy::none()
                    })
                    .user_agent(SECUTILS_USER_AGENT)
                    .build()?;
                let policy_text = match source {
                    ContentSecurityPolicySource::EnforcingHeader
                    | ContentSecurityPolicySource::ReportOnlyHeader => {
                        let response = client.head(url.as_str()).send().await.map_err(|err| SecutilsError::client_with_root_cause(
                            anyhow!(err).context(format!("Cannot fetch content security policy from a web page ({url}) due to unexpected error.")),
                        ))?;

                        let status = response.status();
                        if status.is_client_error() || status.is_server_error() {
                            bail!(SecutilsError::client(format!("Cannot fetch content security policy from a web page ({url}), request failed with HTTP status: {status}.")));
                        }

                        // Extract all values for the specified header, multiple values are allowed,
                        // but Secutils.dev will only import the latest.
                        let header_name = source.header_name();
                        let mut header_values = vec![];
                        for header in response.headers().get_all(header_name) {
                            header_values.push(
                                header
                                    .to_str()
                                    .map_err(|_| {
                                        SecutilsError::client(format!(
                                            "Invalid {header_name} header: {header:?}"
                                        ))
                                    })?
                                    .to_string(),
                            );
                        }

                        if header_values.is_empty() {
                            bail!(SecutilsError::client(format!(
                                "{header_name} header is missing for URL ({url})."
                            )));
                        } else if header_values.len() > 1 {
                            log::warn!(
                                "{header_name} header has {} values for URL ({url}), only the last will be imported: {header_values:?}",
                                header_values.len()
                            );
                        }

                        header_values.remove(header_values.len() - 1)
                    }
                    ContentSecurityPolicySource::Meta => {
                        let response = client.get(url.as_str()).send().await.map_err(|err| SecutilsError::client_with_root_cause(
                            anyhow!(err).context(format!("Cannot fetch content security policy from a web page ({url}) due to unexpected error.")),
                        ))?;

                        let status = response.status();
                        if status.is_client_error() || status.is_server_error() {
                            bail!(SecutilsError::client(format!("Cannot fetch content security policy from a web page ({url}), request failed with HTTP status: {status}.")));
                        }

                        let mut header_values = CspMetaParser::parse(&response.bytes().await?)?;
                        if header_values.is_empty() {
                            bail!(SecutilsError::client(format!(
                                "CSP `<meta>` tag is missing for URL ({url})."
                            )));
                        } else if header_values.len() > 1 {
                            log::warn!(
                                "CSP `<meta>` tag has {} values for URL ({url}), only the last will be imported: {header_values:?}",
                                header_values.len()
                            );
                        }

                        header_values.remove(header_values.len() - 1)
                    }
                };

                let directives = Self::deserialize_directives(&policy_text);
                log::debug!(
                    "Fetched and deserialized {} content security policy directives from URL ({url}): {policy_text}",
                    directives.len()
                );
                directives
            }
        };

        let policy = ContentSecurityPolicy {
            id: Uuid::now_v7(),
            name: params.name,
            directives,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        self.validate_content_security_policy(&policy).await?;

        self.api
            .db
            .web_security()
            .insert_content_security_policy(user_id, &policy)
            .await?;

        Ok(policy)
    }

    /// Updates content security policy.
    pub async fn update_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
        params: ContentSecurityPoliciesUpdateParams,
    ) -> anyhow::Result<ContentSecurityPolicy> {
        if params.name.is_none() && params.directives.is_none() {
            bail!(SecutilsError::client(format!(
                "Either new name, or directives should be provided ({id})."
            )));
        }

        let Some(existing_policy) = self
            .api
            .db
            .web_security()
            .get_content_security_policy(user_id, id)
            .await?
        else {
            bail!(SecutilsError::client(format!(
                "Content security policy ('{id}') is not found."
            )));
        };

        let policy = ContentSecurityPolicy {
            name: params.name.unwrap_or(existing_policy.name),
            directives: params.directives.unwrap_or(existing_policy.directives),
            ..existing_policy
        };

        self.validate_content_security_policy(&policy).await?;

        self.api
            .db
            .web_security()
            .update_content_security_policy(user_id, &policy)
            .await?;

        Ok(policy)
    }

    /// Serializes content security policy.
    pub async fn serialize_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
        params: ContentSecurityPoliciesSerializeParams,
    ) -> anyhow::Result<String> {
        let Some(existing_policy) = self
            .api
            .db
            .web_security()
            .get_content_security_policy(user_id, id)
            .await?
        else {
            bail!(SecutilsError::client(format!(
                "Content security policy ('{id}') is not found."
            )));
        };

        Self::serialize_directives(
            existing_policy
                .directives
                .into_iter()
                .filter(|directive| directive.is_supported_for_source(params.source)),
        )
    }

    /// Removes content security policy by its ID.
    pub async fn remove_content_security_policy(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<()> {
        self.unshare_content_security_policy(user_id, id).await?;
        self.api
            .db
            .web_security()
            .remove_content_security_policy(user_id, id)
            .await
    }

    /// Shares content security policy by its name.
    pub async fn share_content_security_policy(
        &self,
        user_id: UserId,
        policy_id: Uuid,
    ) -> anyhow::Result<UserShare> {
        let users_api = self.api.users();
        let policy_resource = SharedResource::ContentSecurityPolicy { policy_id };

        // Return early if policy is already shared.
        if let Some(user_share) = users_api
            .get_user_share_by_resource(user_id, &policy_resource)
            .await?
        {
            return Ok(user_share);
        }

        // Ensure that policy exists.
        if self
            .get_content_security_policy(user_id, policy_id)
            .await?
            .is_none()
        {
            bail!(SecutilsError::client(format!(
                "Content security policy ('{policy_id}') is not found."
            )));
        }

        // Create new user share.
        let user_share = UserShare {
            id: Default::default(),
            user_id,
            resource: policy_resource,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };
        users_api
            .insert_user_share(&user_share)
            .await
            .map(|_| user_share)
    }

    /// Unshares content security policy by its name.
    pub async fn unshare_content_security_policy(
        &self,
        user_id: UserId,
        policy_id: Uuid,
    ) -> anyhow::Result<Option<UserShare>> {
        let users_api = self.api.users();

        // Check if policy is shared.
        let Some(user_share) = users_api
            .get_user_share_by_resource(
                user_id,
                &SharedResource::ContentSecurityPolicy { policy_id },
            )
            .await?
        else {
            return Ok(None);
        };

        users_api.remove_user_share(user_share.id).await
    }

    async fn validate_content_security_policy(
        &self,
        policy: &ContentSecurityPolicy,
    ) -> anyhow::Result<()> {
        if policy.name.is_empty() {
            bail!(SecutilsError::client(
                "Content security policy name cannot be empty.",
            ));
        }

        if policy.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Content security policy name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        if policy.directives.is_empty() {
            bail!(SecutilsError::client(
                "Content security policy should contain at least on valid directive."
            ));
        }

        Ok(())
    }

    fn deserialize_directives(serialized_policy: &str) -> Vec<ContentSecurityPolicyDirective> {
        // Once policy is parsed, convert it to the internal representation.
        Policy::parse(
            serialized_policy,
            PolicySource::Header,
            PolicyDisposition::Enforce,
        ).directive_set.into_iter().filter_map(|directive| match ContentSecurityPolicyDirective::try_from(&directive) {
            Ok(directive) => Some(directive),
            Err(err) => {
                log::error!("Failed to process parsed content security policy directive ({directive}) due to an error, skipping…: {err}");
                None
            }
        }).collect::<Vec<_>>()
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
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data.
    pub fn web_security(&self) -> WebSecurityApiExt<'_, DR, ET> {
        WebSecurityApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_api, mock_api_with_network, mock_network_with_records, mock_user},
        utils::{
            web_security::api_ext::WebSecurityApiExt, ContentSecurityPoliciesCreateParams,
            ContentSecurityPoliciesSerializeParams, ContentSecurityPoliciesUpdateParams,
            ContentSecurityPolicy, ContentSecurityPolicyContent, ContentSecurityPolicyDirective,
            ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
            ContentSecurityPolicyTrustedTypesDirectiveValue,
        },
    };
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use std::net::Ipv4Addr;
    use trust_dns_resolver::{
        proto::rr::{rdata::A, RData, Record},
        Name,
    };
    use url::Url;
    use uuid::uuid;

    fn get_mock_directives() -> anyhow::Result<Vec<ContentSecurityPolicyDirective>> {
        Ok(vec![
            ContentSecurityPolicyDirective::UpgradeInsecureRequests,
            ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string(), "https://secutils.dev".to_string()]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::TrustedTypes(
                [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::Sandbox(
                [
                    ContentSecurityPolicySandboxDirectiveValue::AllowForms,
                    ContentSecurityPolicySandboxDirectiveValue::AllowPopups,
                ]
                .into_iter()
                .collect(),
            ),
            ContentSecurityPolicyDirective::ReportUri(
                ["https://secutils.dev/report".to_string()]
                    .into_iter()
                    .collect(),
            ),
        ])
    }

    #[tokio::test]
    async fn properly_creates_new_content_security_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        assert_eq!(
            policy,
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_content_security_policy_at_creation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebSecurityApiExt::new(&api);
        let directives = get_mock_directives()?;

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_security_policy(mock_user.id, ContentSecurityPoliciesCreateParams {
                name: "".to_string(),
                content: ContentSecurityPolicyContent::Directives(directives.clone())
            }).await),
            @r###""Content security policy name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_security_policy(mock_user.id, ContentSecurityPoliciesCreateParams {
                name: "a".repeat(101),
                content: ContentSecurityPolicyContent::Directives(directives.clone())
            }).await),
            @r###""Content security policy name cannot be longer than 100 characters.""###
        );

        // Empty directives.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_security_policy(mock_user.id, ContentSecurityPoliciesCreateParams{
                name: "name".to_string(),
                content: ContentSecurityPolicyContent::Directives(vec![])
            }).await),
            @r###""Content security policy should contain at least on valid directive.""###
        );

        // Invalid remote URL schema.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_security_policy(mock_user.id, ContentSecurityPoliciesCreateParams {
                name: "name".to_string(),
                content: ContentSecurityPolicyContent::Remote {
                    url: "ftp://secutils.dev".parse()?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                }
            }).await),
            @r###""Remote URL must be either `http` or `https` and have a valid public reachable domain name, but received ftp://secutils.dev/.""###
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;

        // Non-public URL.
        assert_debug_snapshot!(
            create_and_fail(WebSecurityApiExt::new(&api_with_local_network).create_content_security_policy(mock_user.id, ContentSecurityPoliciesCreateParams {
                name: "name".to_string(),
                 content: ContentSecurityPolicyContent::Remote {
                    url: "https://127.0.0.1".parse()?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                }
            }).await),
            @r###""Remote URL must be either `http` or `https` and have a valid public reachable domain name, but received https://127.0.0.1/.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_content_security_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebSecurityApiExt::new(&api);
        let policy = api
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        // Update name.
        let updated_policy = api
            .update_content_security_policy(
                mock_user.id,
                policy.id,
                ContentSecurityPoliciesUpdateParams {
                    name: Some("name_two".to_string()),
                    directives: None,
                },
            )
            .await?;
        let expected_policy = ContentSecurityPolicy {
            name: "name_two".to_string(),
            ..policy.clone()
        };
        assert_eq!(expected_policy, updated_policy);
        assert_eq!(
            expected_policy,
            api.get_content_security_policy(mock_user.id, policy.id)
                .await?
                .unwrap()
        );

        // Update directives.
        let updated_policy = api
            .update_content_security_policy(
                mock_user.id,
                policy.id,
                ContentSecurityPoliciesUpdateParams {
                    directives: Some(vec![ContentSecurityPolicyDirective::DefaultSrc(
                        ["'none'".to_string()].into_iter().collect(),
                    )]),
                    name: None,
                },
            )
            .await?;
        let expected_policy = ContentSecurityPolicy {
            name: "name_two".to_string(),
            directives: vec![ContentSecurityPolicyDirective::DefaultSrc(
                ["'none'".to_string()].into_iter().collect(),
            )],
            ..policy.clone()
        };
        assert_eq!(expected_policy, updated_policy);
        assert_eq!(
            expected_policy,
            api.get_content_security_policy(mock_user.id, policy.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_content_security_policy_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebSecurityApiExt::new(&api);
        let policy = api
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            api.update_content_security_policy(
                mock_user.id,
                policy.id,
                ContentSecurityPoliciesUpdateParams {
                    name: None,
                    directives: None,
                },
            )
            .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, or directives should be provided ({}).",
                policy.id
            )
        );

        // Non-existent policy.
        let update_result = update_and_fail(
            api.update_content_security_policy(
                mock_user.id,
                uuid!("00000000-0000-0000-0000-000000000002"),
                ContentSecurityPoliciesUpdateParams {
                    name: Some("name".to_string()),
                    directives: None,
                },
            )
            .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Content security policy ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(api.update_content_security_policy(mock_user.id, policy.id, ContentSecurityPoliciesUpdateParams {
                name: Some("".to_string()),
                directives: None,
            }).await),
            @r###""Content security policy name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(api.update_content_security_policy(mock_user.id, policy.id, ContentSecurityPoliciesUpdateParams {
                name: Some("a".repeat(101)),
                directives: None,
            }).await),
            @r###""Content security policy name cannot be longer than 100 characters.""###
        );

        // Empty directive list.
        assert_debug_snapshot!(
            update_and_fail(api.update_content_security_policy(mock_user.id, policy.id, ContentSecurityPoliciesUpdateParams {
                name: None,
                directives: Some(vec![]),
            }).await),
            @r###""Content security policy should contain at least on valid directive.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_new_policy_via_text() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Serialized(
                        "child-src 'self'".to_string(),
                    ),
                },
            )
            .await?;

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-two".to_string(),
                    content: ContentSecurityPolicyContent::Serialized(
                        "script-src 'none'".to_string(),
                    ),
                },
            )
            .await?;

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-two".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ScriptSrc(
                    ["'none'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_new_policy_from_enforcing_header_via_url() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "child-src 'self'");
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_new_policy_from_report_only_header_via_url() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy-Report-Only", "child-src 'self'");
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::ReportOnlyHeader,
                    },
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_new_policy_from_html_meta_via_url() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "script-src 'none'")
                .header("Content-Type", "text/html")
                .body(
                    r#"
                    <!DOCTYPE html>
                    <html>
                        <head><meta http-equiv="Content-Security-Policy" content="child-src 'self'"></head>
                        <body>Hello World!</body>
                    </html>
                    "#,
                );
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::Meta,
                    },
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_new_policy_following_redirect_via_url() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let redirect_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(301).header(
                "Location",
                format!("http://localhost:{}/some-redirected-path", server.port()),
            );
        });
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD)
                .path("/some-redirected-path");
            then.status(200)
                .header("Content-Security-Policy", "child-src 'self'");
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await?;

        redirect_mock.assert();
        web_page_mock.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_last_policy_if_multiple_found() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock_head = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "script-src 'none'")
                .header("Content-Security-Policy", "child-src 'self'");
        });
        let web_page_mock_get = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "script-src 'none'")
                .header("Content-Type", "text/html")
                .body(
                    r#"
                    <!DOCTYPE html>
                    <html>
                        <head>
                            <meta http-equiv="Content-Security-Policy" content="child-src 'self'">
                            <meta http-equiv="Content-Security-Policy" content="script-src 'none'">
                        </head>
                        <body>Hello World!</body>
                    </html>
                    "#,
                );
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await?;

        web_page_mock_head.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-two".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::Meta,
                    },
                },
            )
            .await?;

        web_page_mock_get.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-two".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ScriptSrc(
                    ["'none'".to_string()].into_iter().collect(),
                )],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_imports_ignoring_unknown_directives() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock_head = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200).header(
                "Content-Security-Policy",
                "child-src 'self'; unknown 'unknown; script-src 'none'",
            );
        });
        let web_page_mock_get = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/some-path");
            then.status(200)
                .header("Content-Type", "text/html")
                .body(
                    r#"
                    <!DOCTYPE html>
                    <html>
                        <head>
                            <meta http-equiv="Content-Security-Policy" content="child-src 'self'; unknown 'unknown; script-src 'unsafe-inline'">
                        </head>
                        <body>Hello World!</body>
                    </html>
                    "#,
                );
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await?;

        web_page_mock_head.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    ),
                    ContentSecurityPolicyDirective::ScriptSrc(
                        ["'none'".to_string()].into_iter().collect(),
                    )
                ],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-two".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::Meta,
                    },
                },
            )
            .await?;

        web_page_mock_get.assert();

        assert_eq!(
            policy,
            ContentSecurityPolicy {
                name: "policy-two".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    ),
                    ContentSecurityPolicyDirective::ScriptSrc(
                        ["'unsafe-inline'".to_string()].into_iter().collect(),
                    )
                ],
                ..policy.clone()
            }
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy)
        );

        Ok(())
    }

    #[tokio::test]
    async fn fails_import_if_redirect_required_but_not_permitted() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let redirect_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(301).header(
                "Location",
                format!("{}/some-redirected-path", server.base_url()),
            );
        });
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD)
                .path("/some-redirected-path");
            then.status(200)
                .header("Content-Security-Policy", "child-src 'self'");
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: false,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(import_result.to_string(), format!("content-security-policy header is missing for URL (http://localhost:{}/some-path).", server.port()));

        redirect_mock.assert();
        assert_eq!(web_page_mock.hits(), 0);

        assert!(web_security
            .get_content_security_policies(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn fails_import_if_header_or_html_meta_tag_is_not_found() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock_head = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200);
        });
        let web_page_mock_get = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "script-src 'none'")
                .header("Content-Type", "text/html")
                .body(
                    r###"
                    <!DOCTYPE html>
                    <html>
                        <head>/head>
                        <body>Hello World!</body>
                    </html>
                    "###,
                );
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: false,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(import_result.to_string(), format!("content-security-policy header is missing for URL (http://localhost:{}/some-path).", server.port()));

        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: false,
                        source: ContentSecurityPolicySource::ReportOnlyHeader,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(import_result.to_string(), format!("content-security-policy-report-only header is missing for URL (http://localhost:{}/some-path).", server.port()));

        web_page_mock_head.assert_hits(2);

        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: false,
                        source: ContentSecurityPolicySource::Meta,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(
            import_result.to_string(),
            format!(
                "CSP `<meta>` tag is missing for URL (http://localhost:{}/some-path).",
                server.port()
            )
        );

        web_page_mock_get.assert();

        assert!(web_security
            .get_content_security_policies(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn fails_import_if_request_fails() -> anyhow::Result<()> {
        let api_with_public_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?;

        let mock_user = mock_user()?;
        api_with_public_network.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock_head = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.header("Content-Security-Policy", "script-src 'none'")
                .status(404);
        });
        let web_page_mock_get = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/some-path");
            then.status(404)
                .header("Content-Security-Policy", "script-src 'none'")
                .header("Content-Type", "text/html")
                .body(
                    r#"
                    <!DOCTYPE html>
                    <html>
                        <head><meta http-equiv="Content-Security-Policy" content="child-src 'self'"></head>
                        <body>Hello World!</body>
                    </html>
                    "#,
                );
        });

        let web_security = WebSecurityApiExt::new(&api_with_public_network);
        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: true,
                        source: ContentSecurityPolicySource::EnforcingHeader,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(
            import_result.to_string(),
            format!("Cannot fetch content security policy from a web page (http://localhost:{}/some-path), request failed with HTTP status: 404 Not Found.", server.port())
        );

        web_page_mock_head.assert();

        let import_result = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "policy-one".to_string(),
                    content: ContentSecurityPolicyContent::Remote {
                        // Use `localhost` to trick public domain check logic.
                        url: Url::parse(&format!("http://localhost:{}/some-path", server.port()))?,
                        follow_redirects: false,
                        source: ContentSecurityPolicySource::Meta,
                    },
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;
        assert_eq!(
            import_result.to_string(),
            format!("Cannot fetch content security policy from a web page (http://localhost:{}/some-path), request failed with HTTP status: 404 Not Found.", server.port())
        );

        web_page_mock_get.assert();

        assert!(web_security
            .get_content_security_policies(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_policies() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy_one = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(vec![
                        ContentSecurityPolicyDirective::ChildSrc(
                            ["'self'".to_string()].into_iter().collect(),
                        ),
                    ]),
                },
            )
            .await?;
        let policy_two = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_two".to_string(),
                    content: ContentSecurityPolicyContent::Directives(vec![
                        ContentSecurityPolicyDirective::ChildSrc(
                            ["'none'".to_string()].into_iter().collect(),
                        ),
                    ]),
                },
            )
            .await?;

        assert_eq!(
            web_security
                .get_content_security_policies(mock_user.id)
                .await?,
            [policy_one.clone(), policy_two.clone()]
        );

        web_security
            .remove_content_security_policy(mock_user.id, policy_one.id)
            .await?;
        assert_eq!(
            web_security
                .get_content_security_policies(mock_user.id)
                .await?,
            [policy_two.clone()]
        );

        web_security
            .remove_content_security_policy(mock_user.id, policy_two.id)
            .await?;
        assert!(web_security
            .get_content_security_policies(mock_user.id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_shares_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;
        let policy_share_one = web_security
            .share_content_security_policy(mock_user.id, policy.id)
            .await?;

        assert_eq!(
            api.users().get_user_share(policy_share_one.id).await?,
            Some(policy_share_one.clone())
        );

        // Repetitive sharing should return the same share.
        let policy_share_two = web_security
            .share_content_security_policy(mock_user.id, policy.id)
            .await?;

        assert_eq!(policy_share_one, policy_share_two,);
        assert_eq!(
            api.users().get_user_share(policy_share_one.id).await?,
            Some(policy_share_one.clone())
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_unshares_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;
        let policy_share_one = web_security
            .share_content_security_policy(mock_user.id, policy.id)
            .await?;
        assert_eq!(
            web_security
                .unshare_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy_share_one.clone())
        );

        assert!(api
            .users()
            .get_user_share(policy_share_one.id)
            .await?
            .is_none());

        // Sharing again should return different share.
        let policy_share_two = web_security
            .share_content_security_policy(mock_user.id, policy.id)
            .await?;
        assert_ne!(policy_share_one.id, policy_share_two.id);

        assert_eq!(
            web_security
                .unshare_content_security_policy(mock_user.id, policy.id)
                .await?,
            Some(policy_share_two.clone())
        );

        assert!(api
            .users()
            .get_user_share(policy_share_two.id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn properly_unshares_policy_when_policy_is_removed() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;
        let policy_share = web_security
            .share_content_security_policy(mock_user.id, policy.id)
            .await?;

        assert_eq!(
            api.users().get_user_share(policy_share.id).await?,
            Some(policy_share.clone())
        );

        web_security
            .remove_content_security_policy(mock_user.id, policy.id)
            .await?;

        assert!(web_security
            .get_content_security_policy(mock_user.id, policy.id)
            .await?
            .is_none());
        assert!(api.users().get_user_share(policy_share.id).await?.is_none(),);

        Ok(())
    }

    #[tokio::test]
    async fn properly_serializes_content_security_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApiExt::new(&api);
        let policy = web_security
            .create_content_security_policy(
                mock_user.id,
                ContentSecurityPoliciesCreateParams {
                    name: "name_one".to_string(),
                    content: ContentSecurityPolicyContent::Directives(get_mock_directives()?),
                },
            )
            .await?;

        assert_eq!(
            web_security
                .serialize_content_security_policy(
                    mock_user.id,
                    policy.id,
                    ContentSecurityPoliciesSerializeParams {
                        source: ContentSecurityPolicySource::EnforcingHeader
                    }
                )
                .await?,
            "upgrade-insecure-requests; default-src 'self' https://secutils.dev; trusted-types 'allow-duplicates'; sandbox allow-forms allow-popups; report-uri https://secutils.dev/report"
        );

        assert_eq!(
            web_security
                .serialize_content_security_policy(
                    mock_user.id,
                    policy.id,
                    ContentSecurityPoliciesSerializeParams {
                        source: ContentSecurityPolicySource::ReportOnlyHeader
                    }
                )
                .await?,
            "upgrade-insecure-requests; default-src 'self' https://secutils.dev; trusted-types 'allow-duplicates'; report-uri https://secutils.dev/report"
        );

        assert_eq!(
            web_security
                .serialize_content_security_policy(
                    mock_user.id,
                    policy.id,
                    ContentSecurityPoliciesSerializeParams {
                        source: ContentSecurityPolicySource::Meta
                    }
                )
                .await?,
            "upgrade-insecure-requests; default-src 'self' https://secutils.dev; trusted-types 'allow-duplicates'"
        );

        Ok(())
    }
}
