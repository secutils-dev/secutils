use crate::{
    error::Error as SecutilsError,
    js_runtime::JsRuntime,
    server::app_state::AppState,
    utils::webhooks::{
        ResponderScriptContext, ResponderScriptResult, RespondersRequestCreateParams,
    },
};
use actix_http::{body::MessageBody, StatusCode};
use actix_web::{
    http::header::{HeaderName, HeaderValue},
    web, HttpRequest, HttpResponse,
};
use bytes::Bytes;
use serde::Deserialize;
use std::{borrow::Cow, collections::HashMap};

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

    let responder_method = match request.method().try_into() {
        Ok(responder_method) => responder_method,
        Err(err) => {
            log::error!(
                "Failed to parse HTTP method ({}) into responder method: {:?}",
                request.method(),
                err
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Try to retrieve responder by the name.
    let webhooks = state.api.webhooks();
    let responder = match webhooks
        .find_responder(user.id, &responder_path, responder_method)
        .await
    {
        Ok(Some(responder)) => responder,
        Ok(None) => {
            log::error!(
                "User ('{}') doesn't have an HTTP responder ({} {}) configured.",
                *user.id,
                request.method().as_str(),
                responder_path,
            );
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            log::error!(
                "Failed to retrieve user ({}) HTTP responder ({} {}): {:?}.",
                *user.id,
                request.method().as_str(),
                responder_path,
                err
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Record request
    if responder.settings.requests_to_track > 0 {
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
        webhooks
            .create_responder_request(
                user.id,
                responder.id,
                RespondersRequestCreateParams {
                    client_address: request.peer_addr(),
                    method: Cow::Borrowed(request.method().as_str()),
                    headers: if headers.is_empty() {
                        None
                    } else {
                        Some(headers)
                    },
                    url: Cow::Owned(if let Some(query) = request.uri().query() {
                        format!("{}?{}", responder_path, query)
                    } else {
                        responder_path
                    }),
                    body: if payload.is_empty() {
                        None
                    } else {
                        Some(Cow::Borrowed(&payload))
                    },
                },
            )
            .await?;
    }

    let responder_id = responder.id;

    // Check if body is supposed to be a JavaScript code.
    let (status_code, headers, body) = match responder.settings.script {
        Some(script) => {
            let query = web::Query::<HashMap<String, String>>::from_query(request.query_string())
                .unwrap()
                .into_inner();
            let js_script_context = ResponderScriptContext {
                client_address: request.peer_addr(),
                method: request.method().as_str(),
                headers: request
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.to_str().unwrap_or_default()))
                    .collect(),
                path: request.path(),
                query: query
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect(),
                body: &payload,
            };

            let js_code = format!(r#"(async (globalThis) => {{ return {script}; }})(globalThis);"#);
            let override_result = match JsRuntime::new(&state.config.js_runtime)
                .execute_script::<Option<ResponderScriptResult>>(js_code, Some(js_script_context))
                .await
            {
                Ok(override_result) => override_result.unwrap_or_default(),
                Err(err) => {
                    log::error!(
                        "Failed to execute user ({}) script for HTTP responder ({}): {:?}",
                        *user.id,
                        responder.id,
                        err
                    );
                    return Ok(HttpResponse::InternalServerError().body(err.to_string()));
                }
            };

            (
                override_result
                    .status_code
                    .unwrap_or(responder.settings.status_code),
                override_result
                    .headers
                    .map(|headers| headers.into_iter().collect())
                    .or(responder.settings.headers),
                override_result
                    .body
                    .map(|override_body| override_body.boxed())
                    .or_else(|| responder.settings.body.map(|body| body.boxed())),
            )
        }
        None => (
            responder.settings.status_code,
            responder.settings.headers,
            responder.settings.body.map(|body| body.boxed()),
        ),
    };

    // Prepare response, set response status code.
    let status_code = match StatusCode::from_u16(status_code) {
        Ok(status_code) => status_code,
        Err(err) => {
            log::error!(
                "Failed to parse status code for the user ({}) HTTP responder ({}): {:?}",
                *user.id,
                responder_id,
                err
            );
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    // Prepare response, set response headers.
    let mut response = HttpResponse::new(status_code);
    for (header_name, header_value) in headers.iter().flatten() {
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
                    responder_id,
                    err
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
            (_, Err(err)) => {
                log::error!(
                    "Failed to parse header value {} for the user ({}) HTTP responder ({}): {:?}",
                    header_value,
                    *user.id,
                    responder_id,
                    err
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    }

    // Prepare response, set response body.
    Ok(if let Some(body) = body {
        response.set_body(body)
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
        utils::webhooks::{tests::RespondersCreateParams, ResponderMethod, ResponderSettings},
    };
    use actix_http::{body::MessageBody, Method, Payload};
    use actix_web::{test::TestRequest, web, FromRequest};
    use bytes::Bytes;
    use insta::assert_debug_snapshot;
    use serde_json::json;
    use std::borrow::Cow;

    #[tokio::test]
    async fn can_handle_request_with_path_url_type() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks()
            .create_responder(
                user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/one/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 200,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        script: None,
                    },
                },
            )
            .await?;

        let request = TestRequest::with_uri(
            "https://secutils.dev/api/webhooks/dev-handle-1/one/two?query=value",
        )
        .method(Method::PUT)
        .insert_header(("x-key", "x-value"))
        .insert_header(("x-key-2", "x-value-2"))
        .param("user_handle", "dev-handle-1")
        .param("responder_path", "one/two")
        .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(
            app_state.clone(),
            request,
            Bytes::from_static(b"incoming-body"),
            path,
        )
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

        let mut responder_requests = app_state
            .api
            .webhooks()
            .get_responder_requests(user.id, responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(responder_requests[0].method, "PUT");

        let headers = responder_requests[0].headers.as_mut().unwrap();
        headers.sort_by(|(name1, _), (name2, _)| name1.cmp(name2));
        assert_eq!(
            headers,
            [
                (
                    Cow::Borrowed("x-key"),
                    Cow::Borrowed([120, 45, 118, 97, 108, 117, 101].as_ref())
                ),
                (
                    Cow::Borrowed("x-key-2"),
                    Cow::Borrowed([120, 45, 118, 97, 108, 117, 101, 45, 50].as_ref())
                ),
            ]
            .as_ref()
        );
        assert_eq!(
            responder_requests[0].body,
            Some(Cow::Borrowed(
                [105, 110, 99, 111, 109, 105, 110, 103, 45, 98, 111, 100, 121].as_ref()
            ))
        );
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value")
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_handle_request_with_subdomain_url_type() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks()
            .create_responder(
                user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/one/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 200,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        script: None,
                    },
                },
            )
            .await?;

        let request =
            TestRequest::with_uri("https://dev-handle-1.webhooks.secutils.dev/one/two?query=value")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "dev-handle-1.webhooks.secutils.dev"))
                .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new(), path)
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

        let responder_requests = app_state
            .api
            .webhooks()
            .get_responder_requests(user.id, responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value")
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_handle_request_with_subdomain_url_type_for_root_path() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks()
            .create_responder(
                user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 200,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        script: None,
                    },
                },
            )
            .await?;

        let request = TestRequest::with_uri("https://dev-handle-1.webhooks.secutils.dev")
            .insert_header(("x-replaced-path", "/"))
            .insert_header(("x-forwarded-host", "dev-handle-1.webhooks.secutils.dev"))
            .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new(), path)
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

        let responder_requests = app_state
            .api
            .webhooks()
            .get_responder_requests(user.id, responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn can_handle_responders_with_script() -> anyhow::Result<()> {
        let app_state = mock_app_state().await?;

        // Insert user into the database.
        let user = mock_user()?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks()
            .create_responder(
                user.id,
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    path: "/one/two".to_string(),
                    method: ResponderMethod::Any,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 200,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        script: Some(
                            "(() => { return { statusCode: 300, headers: { one: `two` }, body: Deno.core.encode(JSON.stringify(context)) }; })()".to_string(),
                        ),
                    },
                },
            )
            .await?;

        let request =
            TestRequest::with_uri("https://dev-handle-1.webhooks.secutils.dev/one/two?query=some")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "dev-handle-1.webhooks.secutils.dev"))
                .peer_addr("127.0.0.1:8080".parse()?)
                .to_http_request();
        let path = web::Path::<PathParams>::from_request(&request, &mut Payload::None)
            .await
            .unwrap();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(
            app_state.clone(),
            request,
            Bytes::from_static(b"incoming-body"),
            path,
        )
        .await
        .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 300 Multiple Choices
              headers:
                "one": "two"
              body: Sized(247)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body)?,
            json!({
                "clientAddress": "127.0.0.1:8080",
                "method": "GET",
                "headers": {
                    "x-replaced-path": "/one/two",
                    "x-forwarded-host": "dev-handle-1.webhooks.secutils.dev",
                },
                "path": "/one/two",
                "query": {
                    "query": "some",
                },
                "body": [105, 110, 99, 111, 109, 105, 110, 103, 45, 98, 111, 100, 121],
            })
        );

        let responder_requests = app_state
            .api
            .webhooks()
            .get_responder_requests(user.id, responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);

        Ok(())
    }
}
