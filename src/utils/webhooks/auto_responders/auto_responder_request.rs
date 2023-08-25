use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::IpAddr};
use time::OffsetDateTime;

pub type AutoResponderRequestHeaders<'a> = Vec<(Cow<'a, str>, Cow<'a, [u8]>)>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoResponderRequest<'a> {
    #[serde(rename = "t", with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    pub client_address: Option<IpAddr>,
    #[serde(rename = "m")]
    pub method: Cow<'a, str>,
    #[serde(rename = "h", skip_serializing_if = "Option::is_none")]
    pub headers: Option<AutoResponderRequestHeaders<'a>>,
    #[serde(rename = "b", skip_serializing_if = "Option::is_none")]
    pub body: Option<Cow<'a, [u8]>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::AutoResponderRequest;
    use insta::assert_json_snapshot;
    use std::borrow::Cow;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: None,
            method: Cow::Borrowed("GET"),
            headers: None,
            body: None,
        }, @r###"
        {
          "t": 946720800,
          "m": "GET"
        }
        "###);

        assert_json_snapshot!(AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("Content-Type"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        }, @r###"
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
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<AutoResponderRequest>(r#"{ "t": 946720800, "m": "GET" }"#)?,
            AutoResponderRequest {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                client_address: None,
                method: Cow::Borrowed("GET"),
                headers: None,
                body: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<AutoResponderRequest>(
                r#"{
              "t": 946720800,
              "a": "127.0.0.1",
              "m": "POST",
              "h": [["Content-Type", [1, 2, 3]]],
              "b": [4, 5, 6]
            }"#
            )?,
            AutoResponderRequest {
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
                client_address: Some("127.0.0.1".parse()?),
                method: Cow::Borrowed("POST"),
                headers: Some(vec![(
                    Cow::Borrowed("Content-Type"),
                    Cow::Borrowed(&[1, 2, 3])
                )]),
                body: Some(Cow::Borrowed(&[4, 5, 6])),
            }
        );

        Ok(())
    }
}
