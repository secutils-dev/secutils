use crate::users::SubscriptionFeatures;
use serde_derive::Serialize;
use url::Url;

/// Defines subscription related properties returned as a part of the UI state.
#[derive(Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionState<'u> {
    /// The subscription-dependent features available to the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<SubscriptionFeatures>,
    /// The URL to the subscription management page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manage_url: Option<&'u Url>,
    /// The URL to the subscription overview page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_overview_url: Option<&'u Url>,
}

#[cfg(test)]
mod tests {
    use crate::{
        server::SubscriptionState,
        tests::{mock_config, mock_user},
    };
    use insta::assert_json_snapshot;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(SubscriptionState::default(), @"{}");

        let user = mock_user()?;
        let features = user.subscription.get_features(&mock_config()?);
        let manage_url = Url::parse("http://localhost:1234/subscription")?;
        let feature_overview_url = Url::parse("http://localhost:1234/features")?;

        assert_json_snapshot!(SubscriptionState {
            features: Some(features),
            manage_url: Some(&manage_url),
            feature_overview_url: Some(&feature_overview_url),
        }, @r###"
        {
          "features": {
            "admin": true
          },
          "manageUrl": "http://localhost:1234/subscription",
          "featureOverviewUrl": "http://localhost:1234/features"
        }
        "###);

        Ok(())
    }
}
