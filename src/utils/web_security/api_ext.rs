use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        DictionaryDataUserDataSetter, PublicUserDataNamespace, SharedResource, UserData, UserId,
        UserShare,
    },
    utils::ContentSecurityPolicy,
};
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
            ContentSecurityPolicyDirective,
        },
    };
    use std::collections::HashMap;

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
