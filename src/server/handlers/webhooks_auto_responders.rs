use crate::{
    server::app_state::AppState,
    utils::{AutoResponder, USER_PROFILE_DATA_KEY_AUTO_RESPONDERS},
};
use actix_http::body::MessageBody;
use actix_web::{
    http::header::{HeaderName, HeaderValue},
    web, HttpRequest, HttpResponse, Responder,
};
use reqwest::StatusCode;
use serde_derive::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct PathParams {
    pub user_handle: String,
    pub alias: String,
}

pub async fn webhooks_auto_responders(
    state: web::Data<AppState>,
    request: HttpRequest,
    path_params: web::Path<PathParams>,
) -> impl Responder {
    let PathParams { user_handle, alias } = path_params.into_inner();
    // 1. Try to find a user with such a handle.
    let user = match state.api.users().get_by_handle(&user_handle).await {
        Ok(Some(user)) => user,
        Ok(None) => return HttpResponse::NotFound().finish(),
        Err(err) => {
            log::error!(
                "Failed to retrieve user {} by handle: {:#}",
                user_handle,
                err
            );
            return HttpResponse::InternalServerError()
                .json(json!({ "error": "Responder couldn't handle request. It's likely a bug, please report it." }));
        }
    };

    // 2. Check if user has any responders.
    let auto_responders = user
        .profile
        .and_then(|mut profile| profile.data.take())
        .and_then(|mut profile_data| profile_data.remove(USER_PROFILE_DATA_KEY_AUTO_RESPONDERS));
    let auto_responder = if let Some(auto_responder) = auto_responders {
        auto_responder
    } else {
        log::error!("User {} doesn't have responders configured.", user_handle);
        return HttpResponse::NotFound().finish();
    };

    // 3. Check if user has a responder with a specified alias.
    let auto_responder = match serde_json::from_str::<BTreeMap<String, AutoResponder>>(
        &auto_responder,
    )
    .map(|mut auto_responders| auto_responders.remove(&alias))
    {
        Ok(Some(auto_responder)) => auto_responder,
        Ok(None) => {
            log::error!(
                "User {} doesn't have responder for alias {} configured.",
                user_handle,
                alias
            );
            return HttpResponse::NotFound().finish();
        }
        Err(err) => {
            log::error!(
                "Failed to deserialize responders for user {}: {:#}",
                user_handle,
                err
            );
            return HttpResponse::InternalServerError()
                    .json(json!({ "error": "Responder couldn't handle request. It's likely a bug, please report it." }));
        }
    };

    // 4. Check if responder configured for the HTTP method.
    if !auto_responder.method.matches_http_method(request.method()) {
        log::error!(
            "User {} has responder for alias {} configured, but for another HTTP method, expected: {:?}, actual: {}.",
            user_handle,
            alias,
            auto_responder.method,
            request.method()
        );
        return HttpResponse::NotFound().finish();
    }

    // 5. Create status code.
    let status_code = match StatusCode::from_u16(auto_responder.status_code) {
        Ok(status_code) => status_code,
        Err(err) => {
            log::error!(
                "Failed to parse status code for the user {} responder with alias {}: {:#}",
                user_handle,
                alias,
                err
            );
            return HttpResponse::InternalServerError()
                .json(json!({ "error": "Responder has invalid status code." }));
        }
    };

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
                    "Failed to parse header name {} for the user {} responder with alias {}: {:#}",
                    header_name,
                    user_handle,
                    alias,
                    err
                );
                return HttpResponse::InternalServerError()
                    .json(json!({ "error": "Responder has invalid HTTP response header name." }));
            }
            (_, Err(err)) => {
                log::error!(
                    "Failed to parse header value {} for the user {} responder with alias {}: {:#}",
                    header_value,
                    user_handle,
                    alias,
                    err
                );
                return HttpResponse::InternalServerError()
                    .json(json!({ "error": "Responder has invalid HTTP response header value." }));
            }
        }
    }

    if let Some(body) = auto_responder.body {
        response.set_body(body.boxed())
    } else {
        response
    }
}
