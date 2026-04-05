use crate::{
    config::Config, server::WebhookUrlType, users::User, utils::webhooks::ResponderLocation,
};

/// Returns true when this deployment does not support custom subdomain prefixes on responders
/// (subscription disallows them and/or webhook URLs are path-based). Compute once per
/// import operation and pass the result to the functions that need it.
pub fn should_strip_subdomain_prefixes(config: &Config, user: &User) -> bool {
    if !matches!(config.utils.webhook_url_type, WebhookUrlType::Subdomain) {
        return true;
    }

    !user
        .subscription
        .get_features(config)
        .config
        .webhooks
        .responder_custom_subdomain_prefix
}

/// Returns a clone of `location` with `subdomain_prefix` cleared. Only call when
/// `should_strip_subdomain_prefixes_for_import` returned true **and** the location
/// actually carries a prefix.
pub fn strip_location_subdomain_prefix(location: &ResponderLocation) -> ResponderLocation {
    ResponderLocation {
        subdomain_prefix: None,
        path_type: location.path_type,
        path: location.path.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tests::{MockUserBuilder, mock_config},
        users::{SubscriptionTier, UserSubscription},
        utils::webhooks::ResponderPathType,
    };
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_user() -> User {
        MockUserBuilder::new(
            Uuid::nil().into(),
            "a@b.c",
            "handle",
            datetime!(2010-01-01 0:00 UTC),
        )
        .build()
    }

    fn make_basic_user() -> User {
        MockUserBuilder::new(
            Uuid::nil().into(),
            "a@b.c",
            "handle",
            datetime!(2010-01-01 0:00 UTC),
        )
        .set_subscription(UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: datetime!(2010-01-01 0:00 UTC),
            ends_at: None,
            trial_started_at: None,
            trial_ends_at: None,
        })
        .build()
    }

    #[test]
    fn subdomain_mode_with_feature_enabled_no_strip() -> anyhow::Result<()> {
        let config = mock_config()?;
        assert!(matches!(
            config.utils.webhook_url_type,
            WebhookUrlType::Subdomain
        ));
        assert!(!should_strip_subdomain_prefixes(&config, &make_user()));
        Ok(())
    }

    #[test]
    fn path_mode_requires_strip() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.utils.webhook_url_type = WebhookUrlType::Path;
        assert!(should_strip_subdomain_prefixes(&config, &make_user()));
        Ok(())
    }

    #[test]
    fn subscription_without_feature_requires_strip() -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config
            .subscriptions
            .basic
            .webhooks
            .responder_custom_subdomain_prefix = false;
        assert!(should_strip_subdomain_prefixes(&config, &make_basic_user()));
        Ok(())
    }

    #[test]
    fn strip_clears_prefix_and_preserves_rest() {
        let location = ResponderLocation {
            path_type: ResponderPathType::Exact,
            path: "/hook".to_string(),
            subdomain_prefix: Some("abc".to_string()),
        };
        let stripped = strip_location_subdomain_prefix(&location);
        assert_eq!(stripped.subdomain_prefix, None);
        assert_eq!(stripped.path, location.path);
        assert_eq!(stripped.path_type, location.path_type);
    }

    #[test]
    fn strip_on_location_without_prefix_is_noop() {
        let location = ResponderLocation {
            path_type: ResponderPathType::Prefix,
            path: "/api".to_string(),
            subdomain_prefix: None,
        };
        let stripped = strip_location_subdomain_prefix(&location);
        assert_eq!(stripped, location);
    }
}
