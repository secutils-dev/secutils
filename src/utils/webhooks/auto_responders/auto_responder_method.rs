use actix_web::http;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutoResponderMethod {
    #[serde(rename = "a")]
    Any,
    #[serde(rename = "g")]
    Get,
    #[serde(rename = "p")]
    Post,
    #[serde(rename = "pu")]
    Put,
    #[serde(rename = "d")]
    Delete,
    #[serde(rename = "h")]
    Head,
    #[serde(rename = "o")]
    Options,
    #[serde(rename = "c")]
    Connect,
    #[serde(rename = "t")]
    Trace,
    #[serde(rename = "pa")]
    Patch,
}

impl AutoResponderMethod {
    pub fn matches_http_method(&self, method: &http::Method) -> bool {
        matches!(
            (self, method),
            (AutoResponderMethod::Any, _)
                | (AutoResponderMethod::Get, &http::Method::GET)
                | (AutoResponderMethod::Post, &http::Method::POST)
                | (AutoResponderMethod::Put, &http::Method::PUT)
                | (AutoResponderMethod::Delete, &http::Method::DELETE)
                | (AutoResponderMethod::Head, &http::Method::HEAD)
                | (AutoResponderMethod::Options, &http::Method::OPTIONS)
                | (AutoResponderMethod::Connect, &http::Method::CONNECT)
                | (AutoResponderMethod::Trace, &http::Method::TRACE)
                | (AutoResponderMethod::Patch, &http::Method::PATCH)
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::AutoResponderMethod;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(AutoResponderMethod::Any, @r###""a""###);
        assert_json_snapshot!(AutoResponderMethod::Get, @r###""g""###);
        assert_json_snapshot!(AutoResponderMethod::Post, @r###""p""###);
        assert_json_snapshot!(AutoResponderMethod::Put, @r###""pu""###);
        assert_json_snapshot!(AutoResponderMethod::Delete, @r###""d""###);
        assert_json_snapshot!(AutoResponderMethod::Head, @r###""h""###);
        assert_json_snapshot!(AutoResponderMethod::Options, @r###""o""###);
        assert_json_snapshot!(AutoResponderMethod::Connect, @r###""c""###);
        assert_json_snapshot!(AutoResponderMethod::Trace, @r###""t""###);
        assert_json_snapshot!(AutoResponderMethod::Patch, @r###""pa""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""a""###)?,
            AutoResponderMethod::Any
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""g""###)?,
            AutoResponderMethod::Get
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""p""###)?,
            AutoResponderMethod::Post
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""pu""###)?,
            AutoResponderMethod::Put
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""d""###)?,
            AutoResponderMethod::Delete
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""h""###)?,
            AutoResponderMethod::Head
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""o""###)?,
            AutoResponderMethod::Options
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""c""###)?,
            AutoResponderMethod::Connect
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""t""###)?,
            AutoResponderMethod::Trace
        );
        assert_eq!(
            serde_json::from_str::<AutoResponderMethod>(r###""pa""###)?,
            AutoResponderMethod::Patch
        );

        Ok(())
    }
}
