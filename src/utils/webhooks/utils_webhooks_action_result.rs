use crate::utils::AutoResponderRequest;
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebhooksActionResult {
    #[serde(rename_all = "camelCase")]
    GetAutoRespondersRequests {
        requests: Vec<AutoResponderRequest<'static>>,
    },
}

#[cfg(test)]
mod tests {
    use crate::utils::{AutoResponderRequest, UtilsWebhooksActionResult};
    use insta::assert_json_snapshot;
    use std::borrow::Cow;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsWebhooksActionResult::GetAutoRespondersRequests {
            requests: vec![AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("Content-Type"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        }]
        }, @r###"
        {
          "type": "getAutoRespondersRequests",
          "value": {
            "requests": [
              {
                "t": 946720800,
                "a": "127.0.0.1",
                "m": "GET",
                "h": [
                  [
                    "Content-Type",
                    [
                      1,
                      2,
                      3
                    ]
                  ]
                ],
                "b": [
                  4,
                  5,
                  6
                ]
              }
            ]
          }
        }
        "###);

        Ok(())
    }
}
