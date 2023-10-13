mod csp_meta_parser;

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        DictionaryDataUserDataSetter, PublicUserDataNamespace, SharedResource, UserData, UserId,
        UserShare,
    },
    utils::{
        web_security::api_ext::csp_meta_parser::CspMetaParser, ContentSecurityPolicy,
        ContentSecurityPolicyDirective, ContentSecurityPolicyImportType,
        ContentSecurityPolicySource,
    },
};
use anyhow::{anyhow, bail};
use content_security_policy::{Policy, PolicyDisposition, PolicySource};
use reqwest::redirect::Policy as RedirectPolicy;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub struct WebSecurityApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> WebSecurityApi<'a, DR, ET> {
    /// Creates WebSecurity API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Returns content security policy by its name.
    pub async fn get_content_security_policy(
        &self,
        user_id: UserId,
        policy_name: &str,
    ) -> anyhow::Result<Option<ContentSecurityPolicy>> {
        let users_api = self.api.users();
        Ok(users_api
            .get_data::<BTreeMap<String, ContentSecurityPolicy>>(
                user_id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .and_then(|mut map| map.value.remove(policy_name)))
    }

    /// Upserts content security policy.
    pub async fn upsert_content_security_policy(
        &self,
        user_id: UserId,
        policy: ContentSecurityPolicy,
    ) -> anyhow::Result<()> {
        DictionaryDataUserDataSetter::upsert(
            &self.api.db,
            PublicUserDataNamespace::ContentSecurityPolicies,
            UserData::new(
                user_id,
                [(policy.name.clone(), Some(policy))]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await?;

        Ok(())
    }

    /// Imports content security policy and saves with the specified name.
    pub async fn import_content_security_policy(
        &self,
        user_id: UserId,
        policy_name: String,
        import_type: ContentSecurityPolicyImportType,
    ) -> anyhow::Result<()> {
        // First, fetch policy text if needed.
        let policy_text = match import_type {
            ContentSecurityPolicyImportType::Text { text } => text,
            ContentSecurityPolicyImportType::Url {
                url,
                follow_redirects,
                source,
            } => {
                let client = reqwest::ClientBuilder::new()
                    .redirect(if follow_redirects {
                        RedirectPolicy::default()
                    } else {
                        RedirectPolicy::none()
                    })
                    .build()?;
                match source {
                    ContentSecurityPolicySource::EnforcingHeader
                    | ContentSecurityPolicySource::ReportOnlyHeader => {
                        let response = client.head(url.as_str()).send().await.map_err(|err| {
                            log::error!("Cannot fetch web page headers ({}): {:?}", url, err);
                            anyhow!("Cannot fetch web page ({url}) due to unexpected error.")
                        })?;

                        let status = response.status();
                        if status.is_client_error() || status.is_server_error() {
                            log::error!(
                                "Cannot fetch web page headers ({url}), request failed with HTTP status: {status}."
                            );
                            bail!("Cannot fetch web page headers ({url}), request failed with HTTP status: {status}.");
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
                                        log::error!("Invalid {header_name} header: {header:?}");
                                        anyhow!("Invalid {header_name} header.")
                                    })?
                                    .to_string(),
                            );
                        }

                        if header_values.is_empty() {
                            log::warn!("{header_name} header is missing for URL ({url}).");
                            bail!("{header_name} header is missing.")
                        } else if header_values.len() > 1 {
                            log::warn!(
                                "{header_name} header has {} values for URL ({url}), only the last will be imported: {header_values:?}",
                                header_values.len()
                            );
                        }

                        header_values.remove(header_values.len() - 1)
                    }
                    ContentSecurityPolicySource::Meta => {
                        let response = client.get(url.as_str()).send().await.map_err(|err| {
                            log::error!("Cannot fetch web page ({url}): {err}");
                            anyhow!("Cannot fetch web page ({url}) due to unexpected error.")
                        })?;

                        let status = response.status();
                        if status.is_client_error() || status.is_server_error() {
                            log::error!(
                                "Cannot fetch web page headers ({url}), request failed with HTTP status: {status}."
                            );
                            bail!("Cannot fetch web page headers ({url}), request failed with HTTP status: {status}.");
                        }

                        let mut header_values = CspMetaParser::parse(&response.bytes().await?)?;
                        if header_values.is_empty() {
                            log::warn!("CSP `<meta>` tag is missing for URL ({url}).");
                            bail!("CSP `<meta>` tag is missing.")
                        } else if header_values.len() > 1 {
                            log::warn!(
                                "CSP `<meta>` tag has {} values for URL ({url}), only the last will be imported: {header_values:?}",
                                header_values.len()
                            );
                        }

                        header_values.remove(header_values.len() - 1)
                    }
                }
            }
        };

        // Then, parse the policy.
        let parsed_policy = Policy::parse(
            &policy_text,
            PolicySource::Header,
            PolicyDisposition::Enforce,
        );
        if parsed_policy.directive_set.is_empty() {
            log::error!("Failed to parse content security policy (`{policy_text}).");
            bail!("Failed to parse content security policy.");
        }

        log::debug!(
            "Successfully parsed content security policy ({policy_text}) with the following directives: {:?}",
            parsed_policy.directive_set
        );

        // Once policy is parsed, convert it to the internal representation.
        let directives = parsed_policy.directive_set.into_iter().filter_map(|directive| match ContentSecurityPolicyDirective::try_from(&directive) {
            Ok(directive) => Some(directive),
            Err(err) => {
                log::error!("Failed to process parsed content security policy directive ({directive}) due to an error, skipping…: {err}");
                None
            }
        }).collect::<Vec<_>>();
        if directives.is_empty() {
            log::error!(
                "Processed content security policy ({policy_text}) doesn't have any directives."
            );
            bail!("Failed to process content security policy.");
        }

        self.upsert_content_security_policy(
            user_id,
            ContentSecurityPolicy {
                name: policy_name,
                directives,
            },
        )
        .await
    }

    /// Removes content security policy by its name and returns it.
    pub async fn remove_content_security_policy(
        &self,
        user_id: UserId,
        policy_name: &str,
    ) -> anyhow::Result<()> {
        // 1. Unshare the policy, if it's shared.
        self.unshare_content_security_policy(user_id, policy_name)
            .await?;

        DictionaryDataUserDataSetter::upsert(
            &self.api.db,
            PublicUserDataNamespace::ContentSecurityPolicies,
            UserData::new(
                user_id,
                [(policy_name.to_string(), None)]
                    .into_iter()
                    .collect::<BTreeMap<_, Option<ContentSecurityPolicy>>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await
    }

    /// Shares content security policy by its name.
    pub async fn share_content_security_policy(
        &self,
        user_id: UserId,
        policy_name: &str,
    ) -> anyhow::Result<UserShare> {
        let users_api = self.api.users();
        let policy_resource = SharedResource::ContentSecurityPolicy {
            policy_name: policy_name.to_string(),
        };

        // Return early if policy is already shared.
        if let Some(user_share) = users_api
            .get_user_share_by_resource(user_id, &policy_resource)
            .await?
        {
            return Ok(user_share);
        }

        // Ensure that policy exists.
        if self
            .get_content_security_policy(user_id, policy_name)
            .await?
            .is_none()
        {
            log::error!(
                "Content security policy with name '{}' doesn't exist.",
                policy_name
            );
            anyhow::bail!(
                "Content security policy with name '{}' doesn't exist.",
                policy_name
            );
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
        policy_name: &str,
    ) -> anyhow::Result<Option<UserShare>> {
        let users_api = self.api.users();

        // Check if policy is shared.
        let Some(user_share) = users_api
            .get_user_share_by_resource(
                user_id,
                &SharedResource::ContentSecurityPolicy {
                    policy_name: policy_name.to_string(),
                },
            )
            .await?
        else {
            return Ok(None);
        };

        users_api.remove_user_share(user_share.id).await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data.
    pub fn web_security(&self) -> WebSecurityApi<'_, DR, ET> {
        WebSecurityApi::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api, mock_user},
        users::PublicUserDataNamespace,
        utils::{
            web_security::api_ext::WebSecurityApi, ContentSecurityPolicy,
            ContentSecurityPolicyDirective, ContentSecurityPolicyImportType,
            ContentSecurityPolicySource,
        },
    };
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use std::collections::HashMap;
    use url::Url;

    #[actix_rt::test]
    async fn properly_saves_new_policies() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApi::new(&api);
        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(policy_one.name.clone(), policy_one.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );
        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, &policy_one.name)
                .await?,
            Some(policy_one.clone())
        );

        let policy_two = ContentSecurityPolicy {
            name: "policy-two".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'none'".to_string()].into_iter().collect(),
            )],
        };
        web_security
            .upsert_content_security_policy(mock_user.id, policy_two.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (policy_one.name.clone(), policy_one.clone()),
                (policy_two.name.clone(), policy_two.clone())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_updates_existing_policies() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApi::new(&api);
        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(policy_one.name.clone(), policy_one.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'none'".to_string()].into_iter().collect(),
            )],
        };
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(policy_one.name.clone(), policy_one.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_new_policy_via_text() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Text {
                    text: "child-src 'self'".to_string(),
                },
            )
            .await?;

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Text {
                    text: "script-src 'none'".to_string(),
                },
            )
            .await?;

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ScriptSrc(
                    ["'none'".to_string()].into_iter().collect()
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_new_policy_from_enforcing_header_via_url() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy", "child-src 'self'");
        });

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_new_policy_from_report_only_header_via_url() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let server = MockServer::start();
        let web_page_mock = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD).path("/some-path");
            then.status(200)
                .header("Content-Security-Policy-Report-Only", "child-src 'self'");
        });

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::ReportOnlyHeader,
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_new_policy_from_html_meta_via_url() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::Meta,
                },
            )
            .await?;

        web_page_mock.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_new_policy_following_redirect_via_url() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await?;

        redirect_mock.assert();
        web_page_mock.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_last_policy_if_multiple_found() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await?;

        web_page_mock_head.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            })
        );

        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::Meta,
                },
            )
            .await?;

        web_page_mock_get.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ScriptSrc(
                    ["'none'".to_string()].into_iter().collect(),
                )],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_imports_ignoring_unknown_directives() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await?;

        web_page_mock_head.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    ),
                    ContentSecurityPolicyDirective::ScriptSrc(
                        ["'none'".to_string()].into_iter().collect(),
                    )
                ],
            })
        );

        web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::Meta,
                },
            )
            .await?;

        web_page_mock_get.assert();

        assert_eq!(
            web_security
                .get_content_security_policy(mock_user.id, "policy-one")
                .await?,
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::ChildSrc(
                        ["'self'".to_string()].into_iter().collect(),
                    ),
                    ContentSecurityPolicyDirective::ScriptSrc(
                        ["'unsafe-inline'".to_string()].into_iter().collect(),
                    )
                ],
            })
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn fails_import_if_redirect_required_but_not_permitted() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        let import_result = web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: false,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await;
        assert_debug_snapshot!(import_result, @r###"
        Err(
            "content-security-policy header is missing.",
        )
        "###);

        redirect_mock.assert();
        assert_eq!(web_page_mock.hits(), 0);

        assert!(web_security
            .get_content_security_policy(mock_user.id, "policy-one")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn fails_import_if_header_or_html_meta_tag_is_not_found() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        let import_result = web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await;
        assert_debug_snapshot!(import_result, @r###"
        Err(
            "content-security-policy header is missing.",
        )
        "###);

        let import_result = web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::ReportOnlyHeader,
                },
            )
            .await;
        assert_debug_snapshot!(import_result, @r###"
        Err(
            "content-security-policy-report-only header is missing.",
        )
        "###);

        web_page_mock_head.assert_hits(2);

        let import_result = web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::Meta,
                },
            )
            .await;
        assert_debug_snapshot!(import_result, @r###"
        Err(
            "CSP `<meta>` tag is missing.",
        )
        "###);

        web_page_mock_get.assert();

        assert!(web_security
            .get_content_security_policy(mock_user.id, "policy-one")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn fails_import_if_request_fails() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

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

        let web_security = WebSecurityApi::new(&api);
        assert!(web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::EnforcingHeader,
                },
            )
            .await
            .is_err());

        web_page_mock_head.assert();

        assert!(web_security
            .import_content_security_policy(
                mock_user.id,
                "policy-one".to_string(),
                ContentSecurityPolicyImportType::Url {
                    url: Url::parse(&format!("{}/some-path", server.base_url()))?,
                    follow_redirects: true,
                    source: ContentSecurityPolicySource::Meta,
                },
            )
            .await
            .is_err());

        web_page_mock_get.assert();

        assert!(web_security
            .get_content_security_policy(mock_user.id, "policy-one")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_policies() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };
        let policy_two = ContentSecurityPolicy {
            name: "policy-two".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'none'".to_string()].into_iter().collect(),
            )],
        };

        let web_security = WebSecurityApi::new(&api);
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;
        web_security
            .upsert_content_security_policy(mock_user.id, policy_two.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (policy_one.name.clone(), policy_one.clone()),
                (policy_two.name.clone(), policy_two.clone())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        web_security
            .remove_content_security_policy(mock_user.id, &policy_one.name)
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(policy_two.name.clone(), policy_two.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        web_security
            .remove_content_security_policy(mock_user.id, &policy_two.name)
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?;
        assert!(user_data.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_shares_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };

        // Create and share policy.
        let web_security = WebSecurityApi::new(&api);
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;
        let policy_share_one = web_security
            .share_content_security_policy(mock_user.id, &policy_one.name)
            .await?;

        assert_eq!(
            api.users().get_user_share(policy_share_one.id).await?,
            Some(policy_share_one.clone())
        );

        // Repetitive sharing should return the same share.
        let policy_share_two = web_security
            .share_content_security_policy(mock_user.id, &policy_one.name)
            .await?;

        assert_eq!(policy_share_one, policy_share_two,);
        assert_eq!(
            api.users().get_user_share(policy_share_one.id).await?,
            Some(policy_share_one.clone())
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_unshares_policy() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };

        // Create, share, and unshare policy.
        let web_security = WebSecurityApi::new(&api);
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;
        let policy_share_one = web_security
            .share_content_security_policy(mock_user.id, &policy_one.name)
            .await?;
        assert_eq!(
            web_security
                .unshare_content_security_policy(mock_user.id, &policy_one.name)
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
            .share_content_security_policy(mock_user.id, &policy_one.name)
            .await?;
        assert_ne!(policy_share_one.id, policy_share_two.id);

        assert_eq!(
            web_security
                .unshare_content_security_policy(mock_user.id, &policy_one.name)
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

    #[actix_rt::test]
    async fn properly_unshares_policy_when_policy_is_removed() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let policy_one = ContentSecurityPolicy {
            name: "policy-one".to_string(),
            directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
        };

        // Create and share policy.
        let web_security = WebSecurityApi::new(&api);
        web_security
            .upsert_content_security_policy(mock_user.id, policy_one.clone())
            .await?;
        let policy_share = web_security
            .share_content_security_policy(mock_user.id, &policy_one.name)
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(policy_one.name.clone(), policy_one.clone()),]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        assert_eq!(
            api.users().get_user_share(policy_share.id).await?,
            Some(policy_share.clone())
        );

        web_security
            .remove_content_security_policy(mock_user.id, &policy_one.name)
            .await?;

        let user_data = api
            .users()
            .get_data::<HashMap<String, ContentSecurityPolicy>>(
                mock_user.id,
                PublicUserDataNamespace::ContentSecurityPolicies,
            )
            .await?;
        assert!(user_data.is_none());
        assert!(api.users().get_user_share(policy_share.id).await?.is_none(),);

        Ok(())
    }
}
