mod error_kind;

use actix_web::{http::StatusCode, HttpResponse, HttpResponseBuilder, ResponseError};
use anyhow::anyhow;
use serde_json::json;
use std::fmt::{Debug, Display, Formatter};

pub use error_kind::ErrorKind;

/// Secutils.dev native error type.
#[derive(thiserror::Error)]
pub struct Error {
    root_cause: anyhow::Error,
    kind: ErrorKind,
}

impl Error {
    /// Creates a Client error instance with the given root cause.
    pub fn client_with_root_cause(root_cause: anyhow::Error) -> Self {
        Self {
            root_cause,
            kind: ErrorKind::ClientError,
        }
    }

    /// Creates a Client error instance with the given message.
    pub fn client<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Self {
            root_cause: anyhow!(message),
            kind: ErrorKind::ClientError,
        }
    }

    /// Creates an access forbidden error instance.
    pub fn access_forbidden() -> Self {
        Self {
            root_cause: anyhow!("Access Forbidden"),
            kind: ErrorKind::AccessForbidden,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.root_cause, f)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.root_cause, f)
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self.kind {
            ErrorKind::ClientError => StatusCode::BAD_REQUEST,
            ErrorKind::AccessForbidden => StatusCode::FORBIDDEN,
            ErrorKind::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code()).json(json!({
            "message": match self.kind {
                ErrorKind::ClientError | ErrorKind::AccessForbidden => self.root_cause.to_string(),
                ErrorKind::Unknown => "Internal Server Error".to_string(),
            }
        }))
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Error {
        err.downcast::<Error>().unwrap_or_else(|root_cause| Error {
            root_cause,
            kind: ErrorKind::Unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};
    use actix_web::{body::MessageBody, ResponseError};
    use anyhow::anyhow;
    use bytes::Bytes;
    use insta::assert_debug_snapshot;
    use reqwest::StatusCode;

    #[test]
    fn can_create_client_errors() -> anyhow::Result<()> {
        let error = Error::client("Uh oh.");

        assert_eq!(error.kind, ErrorKind::ClientError);
        assert_debug_snapshot!(error, @r###""Uh oh.""###);

        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);

        let error_response = error.error_response();
        assert_debug_snapshot!(error_response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 400 Bad Request
              headers:
                "content-type": "application/json"
              body: Sized(20)
            ,
        }
        "###);
        let body = error_response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"{\"message\":\"Uh oh.\"}"));

        let error = Error::client_with_root_cause(anyhow!("Something sensitive").context("Uh oh."));

        assert_eq!(error.kind, ErrorKind::ClientError);
        assert_debug_snapshot!(error, @r###"
        Error {
            context: "Uh oh.",
            source: "Something sensitive",
        }
        "###);

        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);

        let error_response = error.error_response();
        assert_debug_snapshot!(error_response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 400 Bad Request
              headers:
                "content-type": "application/json"
              body: Sized(20)
            ,
        }
        "###);
        let body = error_response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"{\"message\":\"Uh oh.\"}"));

        Ok(())
    }

    #[test]
    fn can_create_access_forbidden_errors() -> anyhow::Result<()> {
        let error = Error::access_forbidden();

        assert_eq!(error.kind, ErrorKind::AccessForbidden);
        assert_debug_snapshot!(error, @r###""Access Forbidden""###);

        assert_eq!(error.status_code(), StatusCode::FORBIDDEN);

        let error_response = error.error_response();
        assert_debug_snapshot!(error_response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 403 Forbidden
              headers:
                "content-type": "application/json"
              body: Sized(30)
            ,
        }
        "###);
        let body = error_response.into_body().try_into_bytes().unwrap();
        assert_eq!(
            body,
            Bytes::from_static(b"{\"message\":\"Access Forbidden\"}")
        );

        Ok(())
    }

    #[test]
    fn can_create_unknown_errors() -> anyhow::Result<()> {
        let error = Error::from(anyhow!("Something sensitive"));

        assert_eq!(error.kind, ErrorKind::Unknown);
        assert_debug_snapshot!(error, @r###""Something sensitive""###);

        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);

        let error_response = error.error_response();
        assert_debug_snapshot!(error_response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 500 Internal Server Error
              headers:
                "content-type": "application/json"
              body: Sized(35)
            ,
        }
        "###);
        let body = error_response.into_body().try_into_bytes().unwrap();
        assert_eq!(
            body,
            Bytes::from_static(b"{\"message\":\"Internal Server Error\"}")
        );

        Ok(())
    }

    #[test]
    fn can_recover_original_error() -> anyhow::Result<()> {
        let client_error =
            Error::client_with_root_cause(anyhow!("One").context("Two").context("Three"));
        let error = Error::from(anyhow!(client_error).context("Four"));

        assert_eq!(error.kind, ErrorKind::ClientError);
        assert_debug_snapshot!(error, @r###"
        Error {
            context: "Three",
            source: Error {
                context: "Two",
                source: "One",
            },
        }
        "###);

        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);

        let error_response = error.error_response();
        assert_debug_snapshot!(error_response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 400 Bad Request
              headers:
                "content-type": "application/json"
              body: Sized(19)
            ,
        }
        "###);
        let body = error_response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"{\"message\":\"Three\"}"));

        Ok(())
    }
}
