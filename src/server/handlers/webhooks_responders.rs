use crate::{error::SecutilsError, server::app_state::AppState, utils::AutoResponderRequest};
use actix_http::{body::MessageBody, StatusCode};
use actix_web::{
    http::header::{HeaderName, HeaderValue},
    web, HttpRequest, HttpResponse,
};
use bytes::Bytes;
use serde::Deserialize;
use std::borrow::Cow;
use time::OffsetDateTime;

const X_REPLACED_PATH_HEADER_NAME: &str = "x-replaced-path";

#[derive(Deserialize)]
pub struct PathParams {
    pub user_handle: Option<String>,
    pub responder_path: Option<String>,
}

pub async fn webhooks_responders(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: Bytes,
    path_params: web::Path<PathParams>,
) -> Result<HttpResponse, SecutilsError> {
    let path_params = path_params.into_inner();

    // Extract user handle either from path of from the request headers.
    let user_handle = if let Some(user_handle) = path_params.user_handle {
        user_handle
    } else {
        let connection_info = request.connection_info();
        if let Some(user_handle) = connection_info.host().split('.').next() {
            user_handle.to_string()
        } else {
            log::error!(
                "Failed to extract user handle from host headers ({}) and path ({}).",
                connection_info.host(),
                request.path()
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Extract responder path either from path or from the request headers.
    let mut responder_path = if let Some(responder_path) = path_params.responder_path {
        format!("/{}", responder_path)
    } else {
        let replaced_path = request
            .headers()
            .get(X_REPLACED_PATH_HEADER_NAME)
            .map(|header_value| header_value.to_str())
            .transpose();
        match replaced_path {
            Ok(Some(replaced_path)) => replaced_path.to_string(),
            Ok(None) => {
                log::error!(
                    "Failed to extract responder path from the headers and path ({}).",
                    request.path()
                );
                return Ok(HttpResponse::NotFound().finish());
            }
            Err(err) => {
                log::error!("Failed to parse responder path from headers: {:?}", err);
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    };

    // Try to retrieve use by the handle.
    let user = match state.api.users().get_by_handle(&user_handle).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            log::error!("Failed to find user by the handle ({}).", user_handle);
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            log::error!(
                "Failed to retrieve user by handle ({}) due to unexpected error: {:?}",
                user_handle,
                err
            );
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    // Make sure path doesn't end with trailing slash as it's not allowed.
    if responder_path.len() > 1 && responder_path.ends_with('/') {
        responder_path.pop();
    }

    // Try to retrieve auto responder by the name.
    let auto_responders = state.api.auto_responders();
    let http_responder = match auto_responders
        .get_auto_responder(user.id, &responder_path)
        .await
    {
        Ok(Some(auto_responder)) => auto_responder,
        Ok(None) => {
            log::error!(
                "User ({}) doesn't have HTTP responder ({}) configured.",
                *user.id,
                responder_path
            );
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            log::error!(
                "Failed to retrieve user ({}) HTTP responder ({}): {:?}.",
                *user.id,
                responder_path,
                err
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Check if responder configured for the HTTP method.
    if !http_responder.method.matches_http_method(request.method()) {
        log::error!(
            "User ({}) has HTTP responder ({}) configured, but for another HTTP method, expected: {:?}, actual: {}.",
            *user.id,
            http_responder.path,
            http_responder.method,
            request.method()
        );
        return Ok(HttpResponse::NotFound().finish());
    }

    // Record request
    if http_responder.requests_to_track > 0 {
        let headers = request
            .headers()
            .iter()
            .map(|(header_name, header_value)| {
                (
                    Cow::Borrowed(header_name.as_str()),
                    Cow::Borrowed(header_value.as_bytes()),
                )
            })
            .collect::<Vec<_>>();
        auto_responders
            .track_request(
                user.id,
                &http_responder,
                AutoResponderRequest {
                    timestamp: OffsetDateTime::now_utc(),
                    client_address: request.peer_addr().map(|addr| addr.ip()),
                    method: Cow::Borrowed(request.method().as_str()),
                    headers: if headers.is_empty() {
                        None
                    } else {
                        Some(headers)
                    },
                    body: if payload.is_empty() {
                        None
                    } else {
                        Some(Cow::Borrowed(&payload))
                    },
                },
            )
            .await?;
    }

    // Prepare response, set response status code.
    let status_code = match StatusCode::from_u16(http_responder.status_code) {
        Ok(status_code) => status_code,
        Err(err) => {
            log::error!(
                "Failed to parse status code for the user ({}) HTTP responder ({}): {:?}",
                *user.id,
                http_responder.path,
                err
            );
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    // Prepare response, set response headers.
    let mut response = HttpResponse::new(status_code);
    for (header_name, header_value) in http_responder.headers.iter().flatten() {
        match (
            HeaderName::from_bytes(header_name.as_bytes()),
            HeaderValue::from_str(header_value),
        ) {
            (Ok(header_name), Ok(header_value)) => {
                response.headers_mut().insert(header_name, header_value);
            }
            (Err(err), _) => {
                log::error!(
                    "Failed to parse header name {} for the user ({}) HTTP responder ({}): {:?}",
                    header_name,
                    *user.id,
                    http_responder.path,
                    err
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
            (_, Err(err)) => {
                log::error!(
                    "Failed to parse header value {} for the user ({}) HTTP responder ({}): {:?}",
                    header_value,
                    *user.id,
                    http_responder.path,
                    err
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    }

    // Prepare response, set response body.
    Ok(if let Some(body) = http_responder.body {
        response.set_body(body.boxed())
    } else {
        response
    })
}

#[cfg(test)]
mod tests {
    use super::webhooks_responders;
    use crate::{
        server::handlers::webhooks_responders::PathParams,
        tests::{mock_app_state, mock_user},
        utils::{AutoResponder, AutoResponderMethod},
    };
    use actix_http::{body::MessageBody, Payload};
    use actix_web::{test::TestRequest, web, FromRequest};
    use bytes::Bytes;
    use insta::assert_debug_snapshot;

    #[actix_rt::test]
    async fn can_handle_request_with_path_url_type() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert auto responders data.
        let responder = AutoResponder {
            path: "/one/two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: None,
        };
        app_state
            .api
            .auto_responders()
            .upsert_auto_responder(user.id, responder)
            .await?;

        let request =
            TestRequest::with_uri("https://secutils.dev/api/webhooks/dev-handle-1/one/two")
                .param("user_handle", "dev-handle-1")
                .param("responder_path", "one/two")
                .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new(), path)
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
                "key": "value"
              body: Sized(4)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"body"));

        Ok(())
    }

    #[actix_rt::test]
    async fn can_handle_request_with_subdomain_url_type() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert auto responders data.
        let responder = AutoResponder {
            path: "/one/two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: None,
        };
        app_state
            .api
            .auto_responders()
            .upsert_auto_responder(user.id, responder)
            .await?;

        let request = TestRequest::with_uri("https://dev-handle-1.webhooks.secutils.dev/one/two")
            .insert_header(("x-replaced-path", "/one/two"))
            .insert_header(("x-forwarded-host", "dev-handle-1.webhooks.secutils.dev"))
            .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new(), path)
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
                "key": "value"
              body: Sized(4)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"body"));

        Ok(())
    }

    #[actix_rt::test]
    async fn can_handle_request_with_subdomain_url_type_for_root_path() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert auto responders data.
        let responder = AutoResponder {
            path: "/".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: Some("body".to_string()),
            headers: Some(vec![("key".to_string(), "value".to_string())]),
            delay: None,
        };
        app_state
            .api
            .auto_responders()
            .upsert_auto_responder(user.id, responder)
            .await?;

        let request = TestRequest::with_uri("https://dev-handle-1.webhooks.secutils.dev")
            .insert_header(("x-replaced-path", "/"))
            .insert_header(("x-forwarded-host", "dev-handle-1.webhooks.secutils.dev"))
            .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new(), path)
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
                "key": "value"
              body: Sized(4)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"body"));

        Ok(())
    }
}
