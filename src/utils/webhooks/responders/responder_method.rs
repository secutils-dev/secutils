use actix_web::http::Method;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ResponderMethod {
    Any,
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Trace,
    Patch,
}

impl TryFrom<&Method> for ResponderMethod {
    type Error = anyhow::Error;

    fn try_from(value: &Method) -> Result<Self, Self::Error> {
        match value.as_str() {
            "GET" => Ok(ResponderMethod::Get),
            "POST" => Ok(ResponderMethod::Post),
            "PUT" => Ok(ResponderMethod::Put),
            "DELETE" => Ok(ResponderMethod::Delete),
            "HEAD" => Ok(ResponderMethod::Head),
            "OPTIONS" => Ok(ResponderMethod::Options),
            "CONNECT" => Ok(ResponderMethod::Connect),
            "TRACE" => Ok(ResponderMethod::Trace),
            "PATCH" => Ok(ResponderMethod::Patch),
            method => Err(anyhow::anyhow!("Unsupported HTTP method: {method}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::ResponderMethod;
    use actix_web::http::Method;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ResponderMethod::Any, @r###""ANY""###);
        assert_json_snapshot!(ResponderMethod::Get, @r###""GET""###);
        assert_json_snapshot!(ResponderMethod::Post, @r###""POST""###);
        assert_json_snapshot!(ResponderMethod::Put, @r###""PUT""###);
        assert_json_snapshot!(ResponderMethod::Delete, @r###""DELETE""###);
        assert_json_snapshot!(ResponderMethod::Head, @r###""HEAD""###);
        assert_json_snapshot!(ResponderMethod::Options, @r###""OPTIONS""###);
        assert_json_snapshot!(ResponderMethod::Connect, @r###""CONNECT""###);
        assert_json_snapshot!(ResponderMethod::Trace, @r###""TRACE""###);
        assert_json_snapshot!(ResponderMethod::Patch, @r###""PATCH""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""ANY""#)?,
            ResponderMethod::Any
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""GET""#)?,
            ResponderMethod::Get
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""POST""#)?,
            ResponderMethod::Post
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""PUT""#)?,
            ResponderMethod::Put
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""DELETE""#)?,
            ResponderMethod::Delete
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""HEAD""#)?,
            ResponderMethod::Head
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""OPTIONS""#)?,
            ResponderMethod::Options
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""CONNECT""#)?,
            ResponderMethod::Connect
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""TRACE""#)?,
            ResponderMethod::Trace
        );
        assert_eq!(
            serde_json::from_str::<ResponderMethod>(r#""PATCH""#)?,
            ResponderMethod::Patch
        );

        Ok(())
    }

    #[test]
    fn can_convert_from_http_method() -> anyhow::Result<()> {
        assert_eq!(
            ResponderMethod::try_from(&Method::GET)?,
            ResponderMethod::Get
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::POST)?,
            ResponderMethod::Post
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::PUT)?,
            ResponderMethod::Put
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::DELETE)?,
            ResponderMethod::Delete
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::HEAD)?,
            ResponderMethod::Head
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::OPTIONS)?,
            ResponderMethod::Options
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::CONNECT)?,
            ResponderMethod::Connect
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::TRACE)?,
            ResponderMethod::Trace
        );
        assert_eq!(
            ResponderMethod::try_from(&Method::PATCH)?,
            ResponderMethod::Patch
        );
        assert!(ResponderMethod::try_from(&Method::from_bytes(b"UNSUPPORTED").unwrap()).is_err());

        Ok(())
    }
}
