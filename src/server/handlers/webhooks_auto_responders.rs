use crate::{error::SecutilsError, server::app_state::AppState, utils::AutoResponderRequest};
use actix_http::body::MessageBody;
use actix_web::{
    http::header::{HeaderName, HeaderValue},
    web, HttpRequest, HttpResponse,
};
use bytes::Bytes;
use reqwest::StatusCode;
use serde_derive::Deserialize;
use serde_json::json;
use std::borrow::Cow;
use time::OffsetDateTime;

#[derive(Deserialize)]
pub struct PathParams {
    pub user_handle: String,
    pub name: String,
}

pub async fn webhooks_auto_responders(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: Bytes,
    path_params: web::Path<PathParams>,
) -> Result<HttpResponse, SecutilsError> {
    let PathParams { user_handle, name } = path_params.into_inner();
    // 1. Try to find a user with such a handle.
    let user = match state.api.users().get_by_handle(&user_handle).await {
        Ok(Some(user)) => user,
        Ok(None) => return Ok(HttpResponse::NotFound().finish()),
        Err(err) => {
            log::error!(
                "Failed to retrieve user {} by handle: {:#}",
                user_handle,
                err
            );
            return Ok(HttpResponse::InternalServerError()
                .json(json!({ "error": "Responder couldn't handle request. It's likely a bug, please report it." })));
        }
    };

    // 2. Get auto responder.
    let auto_responder = match state
        .api
        .auto_responders()
        .get_auto_responder(user.id, &name)
        .await
    {
        Ok(Some(auto_responder)) => auto_responder,
        Ok(None) => {
            log::error!(
                "User `{}` doesn't have responder `{}` configured.",
                user_handle,
                name
            );
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            log::error!(
                "Failed to retrieve auto responder `{}` for user `{}`: {:#}.",
                name,
                user_handle,
                err
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // 3. Check if responder configured for the HTTP method.
    if !auto_responder.method.matches_http_method(request.method()) {
        log::error!(
            "User {} has responder for name {} configured, but for another HTTP method, expected: {:?}, actual: {}.",
            user_handle,
            name,
            auto_responder.method,
            request.method()
        );
        return Ok(HttpResponse::NotFound().finish());
    }

    // 4. Record request
    if auto_responder.requests_to_track > 0 {
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
        state
            .api
            .auto_responders()
            .track_request(
                user.id,
                &auto_responder,
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

    // 5. Set response status code.
    let status_code = match StatusCode::from_u16(auto_responder.status_code) {
        Ok(status_code) => status_code,
        Err(err) => {
            log::error!(
                "Failed to parse status code for the user {} responder with name {}: {:#}",
                user_handle,
                name,
                err
            );
            return Ok(HttpResponse::InternalServerError()
                .json(json!({ "error": "Responder has invalid status code." })));
        }
    };

    // 6. Set response headers.
    let mut response = HttpResponse::new(status_code);
    for (header_name, header_value) in auto_responder.headers.iter().flatten() {
        match (
            HeaderName::from_bytes(header_name.as_bytes()),
            HeaderValue::from_str(header_value),
        ) {
            (Ok(header_name), Ok(header_value)) => {
                response.headers_mut().insert(header_name, header_value);
            }
            (Err(err), _) => {
                log::error!(
                    "Failed to parse header name {} for the user {} responder with name {}: {:#}",
                    header_name,
                    user_handle,
                    name,
                    err
                );
                return Ok(HttpResponse::InternalServerError()
                    .json(json!({ "error": "Responder has invalid HTTP response header name." })));
            }
            (_, Err(err)) => {
                log::error!(
                    "Failed to parse header value {} for the user {} responder with name {}: {:#}",
                    header_value,
                    user_handle,
                    name,
                    err
                );
                return Ok(HttpResponse::InternalServerError().json(
                    json!({ "error": "Responder has invalid HTTP response header value." }),
                ));
            }
        }
    }

    // 7. Set response body.
    Ok(if let Some(body) = auto_responder.body {
        response.set_body(body.boxed())
    } else {
        response
    })
}
