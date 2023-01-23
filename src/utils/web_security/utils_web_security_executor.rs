use crate::{
    api::Api,
    users::{User, UserDataType},
    utils::{
        ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicySource,
        UtilsWebSecurityRequest, UtilsWebSecurityResponse,
    },
};
use anyhow::anyhow;
use std::collections::BTreeMap;

fn serialize_directives(
    directives: impl Iterator<Item = ContentSecurityPolicyDirective>,
) -> anyhow::Result<String> {
    let mut serialized_directives = vec![];
    for directive in directives {
        serialized_directives.push(String::try_from(directive)?);
    }

    Ok(serialized_directives.join("; "))
}

pub struct UtilsWebSecurityExecutor;
impl UtilsWebSecurityExecutor {
    pub async fn execute(
        user: User,
        api: &Api,
        request: UtilsWebSecurityRequest,
    ) -> anyhow::Result<UtilsWebSecurityResponse> {
        match request {
            UtilsWebSecurityRequest::SerializeContentSecurityPolicy {
                policy_name,
                source,
            } => {
                let policy = api
                    .users()
                    .get_data::<BTreeMap<String, ContentSecurityPolicy>>(
                        user.id,
                        UserDataType::ContentSecurityPolicies,
                    )
                    .await?
                    .and_then(|mut map| map.remove(&policy_name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find content security policy with name: {}",
                            policy_name
                        )
                    })?;

                let policy = match source {
                    ContentSecurityPolicySource::Meta => serialize_directives(
                        policy
                            .directives
                            .into_iter()
                            .filter(|directive| directive.is_supported_for_source(source)),
                    )?,
                    ContentSecurityPolicySource::Header => {
                        serialize_directives(policy.directives.into_iter())?
                    }
                };

                Ok(UtilsWebSecurityResponse::SerializeContentSecurityPolicy { policy, source })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        web_security::utils_web_security_executor::serialize_directives,
        ContentSecurityPolicyDirective,
    };
    use insta::assert_debug_snapshot;
    use std::collections::HashSet;

    #[test]
    fn can_serialize_directives() -> anyhow::Result<()> {
        let directives = [
            ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string(), "https:".to_string()]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::Sandbox(HashSet::new()),
            ContentSecurityPolicyDirective::ReportTo(["prod-csp".to_string()]),
        ];
        assert_debug_snapshot!(serialize_directives(directives.into_iter())?, @r###""default-src 'self' https:; sandbox; report-to prod-csp""###);

        Ok(())
    }
}
