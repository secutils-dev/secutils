use crate::users::ClientSubscriptionFeatures;
use serde_derive::Serialize;
use url::Url;

/// Defines subscription related properties returned as a part of the UI state.
#[derive(Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionState<'u> {
    /// The features available for the subscription.
    pub features: ClientSubscriptionFeatures<'u>,
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
        let user = mock_user()?;
        let config = mock_config()?;
        let features = user.subscription.get_features(&config);
        let manage_url = Url::parse("http://localhost:1234/subscription")?;
        let feature_overview_url = Url::parse("http://localhost:1234/features")?;

        assert_json_snapshot!(SubscriptionState {
            features: features.into(),
            manage_url: Some(&manage_url),
            feature_overview_url: Some(&feature_overview_url),
        }, @r###"
        {
          "features": {
            "certificates": {},
            "webhooks": {
              "responderRequests": 30
            },
            "webScraping": {
              "trackerRevisions": 30
            },
            "webSecurity": {
              "importPolicyFromUrl": true
            }
          },
          "manageUrl": "http://localhost:1234/subscription",
          "featureOverviewUrl": "http://localhost:1234/features"
        }
        "###);

        Ok(())
    }
}
