use crate::{
    config::Config,
    error::Error as SecutilsError,
    js_runtime::{JsRuntime, JsRuntimeConfig, ProxyState, wrap_script_with_body_conversion},
    server::app_state::AppState,
    utils::webhooks::{
        ResponderScriptContext, ResponderScriptResult, RespondersRequestCreateParams,
    },
};
use actix_web::{
    HttpRequest, HttpResponse,
    body::BoxBody,
    http::{
        StatusCode,
        header::{HeaderName as ActixHeaderName, HeaderValue as ActixHeaderValue},
    },
    web,
};
use anyhow::bail;
use bytes::Bytes;
use serde::Deserialize;
use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

const X_REPLACED_PATH_HEADER_NAME: &str = "x-replaced-path";

struct ResponderOutput {
    status_code: u16,
    headers: Option<Vec<(String, String)>>,
    body: Option<Vec<u8>>,
    skip_request: bool,
    track_response: bool,
}

struct ScriptError {
    response: HttpResponse<BoxBody>,
    error_status_code: u16,
    error_body: Vec<u8>,
}

struct ResponderInfo<'a> {
    resource: &'a str,
    resource_group: &'a str,
    id: uuid::Uuid,
    name: &'a str,
    restrict_to_public_urls: bool,
    max_proxy_response_size: usize,
    max_proxy_request_timeout: std::time::Duration,
}

pub async fn webhooks_responders(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: Bytes,
) -> Result<HttpResponse, SecutilsError> {
    // Extract user handle and subdomain prefix from the request host (set by reverse proxy
    // via X-Forwarded-Host header, e.g. "handle.webhooks.localhost").
    let request_host = {
        let connection_info = request.connection_info();
        connection_info.host().to_string()
    };

    let (user_handle, subdomain_prefix) = match parse_webhook_host(&state.config, &request_host) {
        Ok((user_handle, subdomain_prefix)) => (user_handle, subdomain_prefix),
        Err(err) => {
            error!(
                "Failed to extract user handle and subdomain prefix from the request host ({:?}): {err:?}",
                request_host
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Extract the responder path from the X-Replaced-Path header (set by the reverse proxy,
    // which rewrites the original request path to /api/webhooks).
    let mut responder_path = {
        let replaced_path = request
            .headers()
            .get(X_REPLACED_PATH_HEADER_NAME)
            .map(|header_value| header_value.to_str())
            .transpose();
        match replaced_path {
            Ok(Some(replaced_path)) => replaced_path.to_string(),
            Ok(None) => {
                error!(
                    "Failed to extract responder path from the headers and path ({}).",
                    request.path()
                );
                return Ok(HttpResponse::NotFound().finish());
            }
            Err(err) => {
                error!("Failed to parse responder path from headers: {err:?}");
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    };

    // Try to retrieve use by the handle.
    let user = match state.api.users().get_by_handle(user_handle).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("Failed to find user by the handle ({user_handle}).");
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            error!(
                "Failed to retrieve user by handle ({user_handle}) due to unexpected error: {err:?}"
            );
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    // Make sure path doesn't end with trailing slash as it's not allowed.
    if responder_path.len() > 1 && responder_path.ends_with('/') {
        responder_path.pop();
    }

    // The raw (percent-encoded) responder path is the same as the X-Replaced-Path header value,
    // since the reverse proxy preserves the original URI encoding.
    let raw_responder_path = &responder_path;

    let responder_method = match request.method().try_into() {
        Ok(responder_method) => responder_method,
        Err(err) => {
            error!(
                user.id = %user.id,
                "Failed to parse HTTP method ({}) into responder method: {err:?}",
                request.method()
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    // Try to retrieve responder by the name.
    let webhooks = state.api.webhooks(&user);
    let responder = match webhooks
        .find_responder(subdomain_prefix, &responder_path, responder_method)
        .await
    {
        Ok(Some(responder)) => responder,
        Ok(None) => {
            error!(
                user.id = %user.id,
                "User doesn't have an HTTP responder ({} {subdomain_prefix:?} {responder_path}) configured.",
                request.method().as_str()
            );
            return Ok(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            error!(
                user.id = %user.id,
                "Failed to retrieve HTTP responder ({} {subdomain_prefix:?} {responder_path}): {err:?}.",
                request.method().as_str()
            );
            return Ok(HttpResponse::NotFound().finish());
        }
    };

    let (resource, resource_group) = ("webhooks", "responders");
    if !responder.enabled {
        error!(
            user.id = %user.id,
            util.resource = resource,
            util.resource_group = resource_group,
            util.resource_id = %responder.id,
            util.resource_name = responder.name,
            "User has an HTTP responder ({} {subdomain_prefix:?} {responder_path}) configured, but it is disabled.",
            request.method().as_str(),
        );
        return Ok(HttpResponse::NotFound().finish());
    }

    // Extract logging context before consuming responder to enrich logs.
    let responder_id = responder.id;
    let responder_name = responder.name;

    // Configure subscription limits.
    let subscription_config = user
        .subscription
        .get_features(&state.config)
        .config
        .webhooks;

    // Acquire a concurrency permit for this responder.
    let semaphore = state
        .responder_semaphores
        .entry(responder_id)
        .or_insert_with(|| {
            Arc::new(Semaphore::new(
                subscription_config.max_concurrent_responder_requests,
            ))
        })
        .clone();
    let _permit = match semaphore.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            warn!(
                user.id = %user.id,
                util.resource = resource,
                util.resource_group = resource_group,
                util.resource_id = %responder_id,
                util.resource_name = responder_name,
                "Responder '{}' has reached its concurrent request limit ({}).",
                responder_name,
                subscription_config.max_concurrent_responder_requests,
            );
            return Ok(HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", "1"))
                .body(format!(
                    "Responder '{}' has reached its concurrent request limit ({}). Try again later.",
                    responder_name, subscription_config.max_concurrent_responder_requests,
                )));
        }
    };

    let responder_settings = responder.settings;
    let query = web::Query::<HashMap<String, String>>::from_query(request.query_string())
        .map(|query| query.into_inner())
        .unwrap_or_default();

    let decrypted_secrets = if !responder_settings.secrets.is_none() {
        state
            .api
            .secrets(&user)
            .get_decrypted_secrets(&responder_settings.secrets)
            .await
            .unwrap_or_else(|err| {
                error!(user.id = %user.id, "Failed to decrypt secrets for responder handling: {err:?}");
                HashMap::new()
            })
    } else {
        HashMap::new()
    };

    let default_status_code = responder_settings.status_code;
    let mut default_headers = responder_settings.headers.clone();
    let mut default_body = responder_settings.body.clone().map(String::into_bytes);

    if !decrypted_secrets.is_empty() {
        if let Some(ref mut b) = default_body {
            *b = resolve_secret_templates(
                std::str::from_utf8(b).unwrap_or_default(),
                &decrypted_secrets,
            )
            .into_bytes();
        }
        if let Some(ref mut h) = default_headers {
            for (_, value) in h.iter_mut() {
                if value.contains("${secrets.") {
                    *value = resolve_secret_templates(value, &decrypted_secrets);
                }
            }
        }
    }

    let start = Instant::now();

    let script_outcome: Result<ResponderOutput, ScriptError> =
        if let Some(script) = responder_settings.script.as_ref() {
            let js_script_context = ResponderScriptContext {
                client_address: request.peer_addr(),
                method: request.method().as_str(),
                headers: request
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.to_str().unwrap_or_default()))
                    .collect(),
                path: raw_responder_path,
                raw_query: request.uri().query(),
                query: query
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect(),
                body: &payload,
                secrets: decrypted_secrets,
            };

            let responder_info = ResponderInfo {
                resource,
                resource_group,
                id: responder_id,
                name: &responder_name,
                restrict_to_public_urls: subscription_config.restrict_to_public_urls,
                max_proxy_response_size: subscription_config.max_proxy_response_size,
                max_proxy_request_timeout: subscription_config.max_proxy_request_timeout,
            };
            match execute_responder_script::<Option<ResponderScriptResult>>(
                &state,
                *user.id,
                &subscription_config,
                script,
                &js_script_context,
                &responder_info,
            )
            .await
            {
                Ok(override_result) => {
                    let override_result = override_result.unwrap_or_default();
                    Ok(ResponderOutput {
                        status_code: override_result.status_code.unwrap_or(default_status_code),
                        headers: override_result
                            .headers
                            .map(|headers| headers.into_iter().collect())
                            .or(default_headers),
                        body: override_result
                            .body
                            .map(|body| body.to_vec())
                            .or(default_body),
                        skip_request: override_result.skip_request.unwrap_or(false),
                        track_response: override_result.track_response.unwrap_or(false),
                    })
                }
                Err(err) => Err(err),
            }
        } else {
            Ok(ResponderOutput {
                status_code: default_status_code,
                headers: default_headers,
                body: default_body,
                skip_request: false,
                track_response: false,
            })
        };

    let duration_ms = Some(start.elapsed().as_millis() as u32);
    let max_response_size = subscription_config.max_tracked_response_size;

    let skip_request = match &script_outcome {
        Ok(output) => output.skip_request,
        Err(_) => false,
    };

    if !skip_request {
        let tracked_headers = request
            .headers()
            .iter()
            .map(|(header_name, header_value)| {
                (
                    Cow::Borrowed(header_name.as_str()),
                    Cow::Borrowed(header_value.as_bytes()),
                )
            })
            .collect::<Vec<_>>();

        let (resp_status, resp_headers, resp_body) = match &script_outcome {
            Ok(output) if output.track_response => {
                let truncated_body = output.body.as_ref().map(|b| {
                    if b.len() > max_response_size {
                        Cow::Owned(b[..max_response_size].to_vec())
                    } else {
                        Cow::Borrowed(b.as_slice())
                    }
                });
                let resp_headers: Option<Vec<_>> = output.headers.as_ref().map(|h| {
                    h.iter()
                        .map(|(k, v)| (Cow::Borrowed(k.as_str()), Cow::Borrowed(v.as_bytes())))
                        .collect()
                });
                (Some(output.status_code), resp_headers, truncated_body)
            }
            Err(script_err) => {
                let body = if script_err.error_body.len() > max_response_size {
                    script_err.error_body[..max_response_size].to_vec()
                } else {
                    script_err.error_body.clone()
                };
                (
                    Some(script_err.error_status_code),
                    Some(vec![(
                        Cow::Borrowed("content-type"),
                        Cow::Borrowed(b"text/plain; charset=utf-8".as_slice()),
                    )]),
                    Some(Cow::Owned(body)),
                )
            }
            _ => (None, None, None),
        };

        webhooks
            .create_responder_request(
                responder_id,
                RespondersRequestCreateParams {
                    client_address: request.peer_addr(),
                    method: Cow::Borrowed(request.method().as_str()),
                    headers: if tracked_headers.is_empty() {
                        None
                    } else {
                        Some(tracked_headers)
                    },
                    url: Cow::Owned(if let Some(query) = request.uri().query() {
                        format!("{raw_responder_path}?{query}")
                    } else {
                        raw_responder_path.to_string()
                    }),
                    body: if payload.is_empty() {
                        None
                    } else {
                        Some(Cow::Borrowed(&payload))
                    },
                    duration_ms,
                    response_status_code: resp_status,
                    response_headers: resp_headers,
                    response_body: resp_body,
                },
            )
            .await?;
    }

    let ResponderOutput {
        status_code,
        headers,
        body,
        ..
    } = match script_outcome {
        Ok(output) => output,
        Err(script_err) => return Ok(script_err.response),
    };

    // Prepare response, set response status code.
    let status_code = match StatusCode::from_u16(status_code) {
        Ok(status_code) => status_code,
        Err(err) => {
            error!(
                user.id = %user.id,
                util.resource = resource,
                util.resource_group = resource_group,
                util.resource_id = %responder_id,
                util.resource_name = responder_name,
                "Failed to parse status code for the HTTP responder: {err:?}",
            );
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    // Prepare response, set response headers.
    let mut response = HttpResponse::build(status_code);
    for (header_name, header_value) in headers.iter().flatten() {
        match (
            ActixHeaderName::from_bytes(header_name.as_bytes()),
            ActixHeaderValue::from_str(header_value),
        ) {
            (Ok(header_name), Ok(header_value)) => {
                response.insert_header((header_name, header_value));
            }
            (Err(err), _) => {
                error!(
                    user.id = %user.id,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    util.resource_id = %responder_id,
                    util.resource_name = responder_name,
                    "Failed to parse header name `{header_name}` for the HTTP responder: {err:?}"
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
            (_, Err(err)) => {
                error!(
                    user.id = %user.id,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    util.resource_id = %responder_id,
                    util.resource_name = responder_name,
                    "Failed to parse header value `{header_value}` for the HTTP responder: {err:?}"
                );
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    }

    // Prepare response, set response body.
    Ok(if let Some(body) = body {
        response.body(body)
    } else {
        response.finish()
    })
}

async fn execute_responder_script<R: for<'de> Deserialize<'de> + Send + 'static>(
    state: &web::Data<AppState>,
    user_id: uuid::Uuid,
    subscription_config: &crate::config::SubscriptionWebhooksConfig,
    script: &str,
    js_script_context: &ResponderScriptContext<'_>,
    responder: &ResponderInfo<'_>,
) -> Result<R, ScriptError> {
    let js_runtime_config = JsRuntimeConfig {
        max_heap_size: subscription_config.js_runtime_heap_size,
        max_user_script_execution_time: subscription_config.js_runtime_script_execution_time,
    };

    let proxy_state = ProxyState::new(
        Arc::new(state.api.network.clone()),
        responder.restrict_to_public_urls,
        responder.max_proxy_response_size,
        responder.max_proxy_request_timeout,
    );

    let js_code = wrap_script_with_body_conversion(script);
    let js_script_context_json = match serde_json::to_string(js_script_context) {
        Ok(json) => json,
        Err(err) => {
            error!(
                user.id = %user_id,
                util.resource = responder.resource,
                util.resource_group = responder.resource_group,
                util.resource_id = %responder.id,
                util.resource_name = responder.name,
                "Failed to serialize responder script context: {err:?}"
            );
            let msg = "Failed to serialize script context".to_string();
            return Err(ScriptError {
                error_status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error_body: msg.as_bytes().to_vec(),
                response: HttpResponse::InternalServerError().body(msg),
            });
        }
    };

    match JsRuntime::execute_script::<R>(
        js_runtime_config,
        js_code,
        Some(js_script_context_json),
        Some(proxy_state),
    )
    .await
    {
        Ok((script_result, execution_time)) => {
            info!(
                user.id = %user_id,
                util.resource = responder.resource,
                util.resource_group = responder.resource_group,
                util.resource_id = %responder.id,
                util.resource_name = responder.name,
                metrics.script_execution_time = execution_time.as_nanos() as u64,
                "Executed responder user script in {execution_time:.2?}.",
            );
            Ok(script_result)
        }
        Err(err) => {
            let err_msg = err.to_string();
            error!(
                user.id = %user_id,
                util.resource = responder.resource,
                util.resource_group = responder.resource_group,
                util.resource_id = %responder.id,
                util.resource_name = responder.name,
                "Failed to execute responder user script: {err:?}"
            );

            let (status, response) = if err_msg.contains("Script exceeded time limit")
                || err_msg.contains("Upstream request timed out")
            {
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    HttpResponse::GatewayTimeout().body(err_msg.clone()),
                )
            } else if err_msg.contains("URL not allowed")
                || err_msg.contains("Failed to connect to upstream")
                || err_msg.contains("Upstream request failed")
            {
                (
                    StatusCode::BAD_GATEWAY,
                    HttpResponse::BadGateway().body(err_msg.clone()),
                )
            } else if err_msg.contains("Upstream response body too large") {
                (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    HttpResponse::PayloadTooLarge().body(err_msg.clone()),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HttpResponse::InternalServerError().body(err_msg.clone()),
                )
            };
            Err(ScriptError {
                error_status_code: status.as_u16(),
                error_body: err_msg.into_bytes(),
                response,
            })
        }
    }
}

/// Replaces `${secrets.KEY}` patterns in a string with the corresponding decrypted secret values.
/// Unresolved references (missing secrets) are left as-is and logged as warnings.
fn resolve_secret_templates(input: &str, secrets: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    let mut start = 0;
    while let Some(begin) = result[start..].find("${secrets.") {
        let abs_begin = start + begin;
        let after_prefix = abs_begin + "${secrets.".len();
        if let Some(end_offset) = result[after_prefix..].find('}') {
            let key = &result[after_prefix..after_prefix + end_offset];
            if let Some(value) = secrets.get(key) {
                let full_pattern_end = after_prefix + end_offset + 1;
                result.replace_range(abs_begin..full_pattern_end, value);
                start = abs_begin + value.len();
            } else {
                warn!("Unresolved secret reference: ${{secrets.{key}}}");
                start = after_prefix + end_offset + 1;
            }
        } else {
            break;
        }
    }
    result
}

/// Parses the host that webhook was access through to determine user handle and subdomain prefix.
pub fn parse_webhook_host<'s>(
    config: &Config,
    webhook_host: &'s str,
) -> anyhow::Result<(&'s str, Option<&'s str>)> {
    let Some(public_host) = config.public_url.host_str() else {
        bail!(SecutilsError::client(
            "Public URL doesn't have a host, cannot extract responder subdomain prefix."
        ));
    };

    // Strip port if present (e.g. "handle.webhooks.localhost:7171" → "handle.webhooks.localhost").
    let webhook_host = webhook_host
        .rsplit_once(':')
        .map_or(webhook_host, |(host, _port)| host);

    // First remove the public URL host from the request host to keep only user-specific part.
    let Some(webhook_subdomain) = webhook_host.strip_suffix(&format!(".webhooks.{public_host}"))
    else {
        bail!(SecutilsError::client(format!(
            "Failed to extract base host from the webhook host ({webhook_host})."
        )));
    };

    // Next separate user handle part from the rest of the subdomain, e.g.,:
    // abc-user-handle.secutils.dev -> (user-handle, Some("abc"))
    Ok(match webhook_subdomain.rsplit_once('-') {
        // No custom subdomain, just user handle.
        None => (webhook_subdomain, None),
        Some((subdomain_prefix, user_handle)) => (user_handle, Some(subdomain_prefix)),
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_webhook_host, webhooks_responders};
    use crate::{
        tests::{mock_app_state, mock_config, mock_user},
        users::SecretsAccess,
        utils::webhooks::{
            ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
            tests::{RespondersCreateParams, RespondersUpdateParams},
        },
    };
    use actix_web::{body::MessageBody, http::StatusCode, test::TestRequest, web};
    use bytes::Bytes;
    use insta::assert_debug_snapshot;
    use serde_json::json;
    use sqlx::PgPool;
    use std::{borrow::Cow, default::Default};

    #[sqlx::test]
    async fn can_handle_request(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        // Insert user into the database.
        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/one/two".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request =
            TestRequest::with_uri("https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/one/two?query=value")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "devhandle00000000000000000000000000000001.webhooks.secutils.dev"))
                .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
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
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_handle_request_for_root_path(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        // Insert user into the database.
        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        )
        .insert_header(("x-replaced-path", "/"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
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
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn can_handle_request_with_custom_subdomain(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        // Insert user into the database.
        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        // Insert responders data.
        let responder_one = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/one/two".to_string(),
                    subdomain_prefix: Some("abc".to_string()),
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;
        let responder_two = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "name_two".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/one/two".to_string(),
                    subdomain_prefix: Some("cba".to_string()),
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body-two".to_string()),
                    headers: Some(vec![("key-2".to_string(), "value-2".to_string())]),
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request =
            TestRequest::with_uri("https://abc-devhandle00000000000000000000000000000001.webhooks.secutils.dev/one/two?query=value")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "abc-devhandle00000000000000000000000000000001.webhooks.secutils.dev"))
                .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
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
            .webhooks(&user)
            .get_responder_requests(responder_one.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value")
        );

        let request =
            TestRequest::with_uri("https://cba-devhandle00000000000000000000000000000001.webhooks.secutils.dev/one/two?query=value-2")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "cba-devhandle00000000000000000000000000000001.webhooks.secutils.dev"))
                .to_http_request();
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
                "key-2": "value-2"
              body: Sized(8)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"body-two"));

        let responder_requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder_two.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value-2")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_handle_responders_with_script(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        // Insert user into the database.
        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(
                RespondersCreateParams {
                    name: "name_one".to_string(),
                    location: ResponderLocation {
                        path_type: ResponderPathType::Exact,
                        path: "/one/two".to_string(),
                        subdomain_prefix: None
                    },
                    method: ResponderMethod::Any,
                    enabled: true,
                    settings: ResponderSettings {
                        requests_to_track: 3,
                        status_code: 200,
                        body: Some("body".to_string()),
                        headers: Some(vec![("key".to_string(), "value".to_string())]),
                        script: Some(
                            "(() => { return { statusCode: 300, headers: { one: `two` }, body: Deno.core.encode(JSON.stringify(context)) }; })()".to_string(),
                        ),
                        secrets: SecretsAccess::None,
                    },
                    tag_ids: vec![],
                },
            )
            .await?;

        let request =
            TestRequest::with_uri("https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/one/two?query=some")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "devhandle00000000000000000000000000000001.webhooks.secutils.dev"))
                .peer_addr("127.0.0.1:8080".parse()?)
                .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(
            app_state.clone(),
            request,
            Bytes::from_static(b"incoming-body"),
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
              body: Sized(300)
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
                    "x-forwarded-host": "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
                },
                "path": "/one/two",
                "rawQuery": "query=some",
                "query": {
                    "query": "some",
                },
                "body": [105, 110, 99, 111, 109, 105, 110, 103, 45, 98, 111, 100, 121],
            })
        );

        let responder_requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn script_context_omits_raw_query_when_absent(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "no_query".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/no-qs".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { return { body: Deno.core.encode(JSON.stringify(context)) }; })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/no-qs",
        )
        .insert_header(("x-replaced-path", "/no-qs"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();

        let body = response.into_body().try_into_bytes().unwrap();
        let ctx: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(ctx.get("rawQuery"), None);
        assert_eq!(ctx["path"], "/no-qs");
        assert_eq!(ctx["query"], json!({}));

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_non_existent_or_inactive_responders(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let request =
            TestRequest::with_uri("https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/one/two?query=value")
                .insert_header(("x-replaced-path", "/one/two"))
                .insert_header(("x-forwarded-host", "devhandle00000000000000000000000000000001.webhooks.secutils.dev"))
                .to_http_request();
        let app_state = mock_app_state(pool).await?;
        let app_state = web::Data::new(app_state);

        // 1. Non-existent user handle.
        let response = webhooks_responders(app_state.clone(), request.clone(), Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        // 2. Non-existent responder.
        let response = webhooks_responders(app_state.clone(), request.clone(), Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        // Insert responders data.
        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "name_one".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/one/two".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: false,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        // 3. Inactive responder.
        let response = webhooks_responders(app_state.clone(), request.clone(), Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);
        let responder_requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert!(responder_requests.is_empty());

        app_state
            .api
            .webhooks(&user)
            .update_responder(
                responder.id,
                RespondersUpdateParams {
                    enabled: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        // 4. Active responder.
        let response = webhooks_responders(app_state.clone(), request.clone(), Bytes::new())
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
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(responder_requests.len(), 1);
        assert_eq!(
            responder_requests[0].url,
            Cow::Borrowed("/one/two?query=value")
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_parse_webhook_hosts() -> anyhow::Result<()> {
        let test_cases = [
            ("a-handle.webhooks.secutils.dev", ("handle", Some("a"))),
            (
                "my-sub-handle.webhooks.secutils.dev",
                ("handle", Some("my-sub")),
            ),
            ("abc-handle.webhooks.secutils.dev", ("handle", Some("abc"))),
            (
                "a1-b-d-com-handle.webhooks.secutils.dev",
                ("handle", Some("a1-b-d-com")),
            ),
            ("handle.webhooks.secutils.dev", ("handle", None)),
        ];

        let config = mock_config()?;
        for (webhook_host, expected_result) in test_cases {
            assert_eq!(parse_webhook_host(&config, webhook_host)?, expected_result);
        }

        Ok(())
    }

    #[sqlx::test]
    async fn script_timeout_returns_gateway_timeout(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "timeout_test".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/timeout".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Script exceeded time limit'); })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/timeout",
        )
        .insert_header(("x-replaced-path", "/timeout"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::GATEWAY_TIMEOUT
        );

        let body = response.into_body().try_into_bytes().unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("Script exceeded time limit"),
        );

        Ok(())
    }

    #[sqlx::test]
    async fn script_upstream_timeout_returns_gateway_timeout(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "upstream_timeout".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/up-timeout".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Upstream request timed out'); })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/up-timeout",
        )
        .insert_header(("x-replaced-path", "/up-timeout"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::GATEWAY_TIMEOUT
        );

        Ok(())
    }

    #[sqlx::test]
    async fn script_url_not_allowed_returns_bad_gateway(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "ssrf_test".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/ssrf".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('URL not allowed: http://169.254.169.254'); })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/ssrf",
        )
        .insert_header(("x-replaced-path", "/ssrf"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        let body = response.into_body().try_into_bytes().unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("URL not allowed")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn script_connect_failure_returns_bad_gateway(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "connect_fail".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/connect-fail".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Failed to connect to upstream'); })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/connect-fail",
        )
        .insert_header(("x-replaced-path", "/connect-fail"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        Ok(())
    }

    #[sqlx::test]
    async fn script_upstream_request_failed_returns_bad_gateway(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "upstream_fail".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/up-fail".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Upstream request failed'); })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/up-fail",
        )
        .insert_header(("x-replaced-path", "/up-fail"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        Ok(())
    }

    #[sqlx::test]
    async fn script_response_too_large_returns_payload_too_large(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "too_large".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/too-large".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Upstream response body too large: 20971520 bytes exceeds limit'); })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/too-large",
        )
        .insert_header(("x-replaced-path", "/too-large"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::PAYLOAD_TOO_LARGE
        );

        let body = response.into_body().try_into_bytes().unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("Upstream response body too large"),
        );

        Ok(())
    }

    #[sqlx::test]
    async fn script_generic_error_returns_internal_server_error(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "generic_err".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/generic-err".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Something unexpected happened'); })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/generic-err",
        )
        .insert_header(("x-replaced-path", "/generic-err"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let body = response.into_body().try_into_bytes().unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("Something unexpected happened"),
        );

        Ok(())
    }

    #[sqlx::test]
    async fn concurrent_request_limit_returns_too_many_requests(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "limited".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/limited".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: Some("ok".to_string()),
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let app_state = web::Data::new(app_state);

        // Pre-fill the semaphore to exhaust all permits (default limit = 10).
        let semaphore = Arc::new(Semaphore::new(10));
        let _permits: Vec<_> = (0..10)
            .map(|_| semaphore.clone().try_acquire_owned().unwrap())
            .collect();
        app_state
            .responder_semaphores
            .insert(responder.id, semaphore);

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/limited",
        )
        .insert_header(("x-replaced-path", "/limited"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok()),
            Some("1")
        );

        let body = response.into_body().try_into_bytes().unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert!(body_str.contains("limited"));
        assert!(body_str.contains("concurrent request limit"));

        Ok(())
    }

    #[sqlx::test]
    async fn concurrent_request_succeeds_when_permits_available(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "available".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/available".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: Some("ok".to_string()),
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/available",
        )
        .insert_header(("x-replaced-path", "/available"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"ok"));

        Ok(())
    }

    #[sqlx::test]
    async fn script_returning_null_uses_defaults(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "null_script".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/null-script".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 201,
                    body: Some("default-body".to_string()),
                    headers: Some(vec![("x-default".to_string(), "yes".to_string())]),
                    script: Some("(() => { return null; })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/null-script",
        )
        .insert_header(("x-replaced-path", "/null-script"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 201 Created
              headers:
                "x-default": "yes"
              body: Sized(12)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"default-body"));

        Ok(())
    }

    #[sqlx::test]
    async fn script_partial_override_merges_with_defaults(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "partial_override".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/partial".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 0,
                    status_code: 200,
                    body: Some("default-body".to_string()),
                    headers: Some(vec![("x-default".to_string(), "yes".to_string())]),
                    script: Some("(() => { return { statusCode: 202 }; })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/partial",
        )
        .insert_header(("x-replaced-path", "/partial"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let response = webhooks_responders(web::Data::new(app_state), request, Bytes::new())
            .await
            .unwrap();
        // Script overrides status code to 202 but doesn't override headers/body, so defaults apply.
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 202 Accepted
              headers:
                "x-default": "yes"
              body: Sized(12)
            ,
        }
        "###);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(body, Bytes::from_static(b"default-body"));

        Ok(())
    }

    #[sqlx::test]
    async fn script_skip_request_suppresses_tracking(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "skip_tracking".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/skip".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("ok".to_string()),
                    headers: None,
                    script: Some(
                        "(() => { return { statusCode: 204, skipRequest: true }; })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/skip",
        )
        .insert_header(("x-replaced-path", "/skip"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 204 No Content
              headers:
              body: Sized(2)
            ,
        }
        "###);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 0);

        Ok(())
    }

    #[sqlx::test]
    async fn script_skip_request_false_tracks_normally(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "explicit_track".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/track".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("ok".to_string()),
                    headers: None,
                    script: Some("(() => { return { skipRequest: false }; })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/track",
        )
        .insert_header(("x-replaced-path", "/track"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn script_without_skip_request_tracks_by_default(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "default_track".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/default".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("ok".to_string()),
                    headers: None,
                    script: Some("(() => { return { statusCode: 200 }; })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/default",
        )
        .insert_header(("x-replaced-path", "/default"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn script_failure_still_tracks_request(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "failing_script".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/fail".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { throw new Error('Script exceeded time limit'); })()".to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/fail",
        )
        .insert_header(("x-replaced-path", "/fail"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].response_status_code.is_some());
        assert!(requests[0].response_body.is_some());
        assert!(requests[0].duration_ms.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn script_track_response_stores_response(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "track_resp".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/track".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("default-body".to_string()),
                    headers: Some(vec![("X-Custom".to_string(), "val".to_string())]),
                    script: Some(
                        "(() => { return { statusCode: 201, headers: { 'X-Resp': 'yes' }, body: new Uint8Array([72, 73]), trackResponse: true }; })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/track",
        )
        .insert_header(("x-replaced-path", "/track"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].response_status_code, Some(201));
        assert!(requests[0].response_headers.is_some());
        let resp_headers = requests[0].response_headers.as_ref().unwrap();
        assert_eq!(resp_headers.len(), 1);
        assert_eq!(resp_headers[0].0, "X-Resp");
        assert_eq!(
            requests[0].response_body,
            Some(Cow::Borrowed(&[72, 73][..]))
        );
        assert!(requests[0].duration_ms.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn script_track_response_default_no_response_data(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "no_track".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/notrack".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("body".to_string()),
                    headers: None,
                    script: Some("(() => { return {}; })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/notrack",
        )
        .insert_header(("x-replaced-path", "/notrack"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].response_status_code.is_none());
        assert!(requests[0].response_headers.is_none());
        assert!(requests[0].response_body.is_none());
        assert!(requests[0].duration_ms.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn script_skip_request_with_track_response_skips_everything(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "skip_and_track".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/skiptrack".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some(
                        "(() => { return { skipRequest: true, trackResponse: true, statusCode: 200 }; })()"
                            .to_string(),
                    ),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/skiptrack",
        )
        .insert_header(("x-replaced-path", "/skiptrack"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 0);

        Ok(())
    }

    #[sqlx::test]
    async fn script_failure_auto_tracks_error_response(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "auto_track_error".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/autofail".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: Some("(() => { throw new Error('something broke'); })()".to_string()),
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/autofail",
        )
        .insert_header(("x-replaced-path", "/autofail"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].response_status_code, Some(500));
        assert!(requests[0].response_body.is_some());
        let body_bytes = requests[0].response_body.as_ref().unwrap();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("something broke"));
        assert!(requests[0].duration_ms.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn no_script_responder_records_duration(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let responder = app_state
            .api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "static_resp".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/static".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: Some("hello".to_string()),
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                },
                tag_ids: vec![],
            })
            .await?;

        let request = TestRequest::with_uri(
            "https://devhandle00000000000000000000000000000001.webhooks.secutils.dev/static",
        )
        .insert_header(("x-replaced-path", "/static"))
        .insert_header((
            "x-forwarded-host",
            "devhandle00000000000000000000000000000001.webhooks.secutils.dev",
        ))
        .to_http_request();
        let app_state = web::Data::new(app_state);
        let response = webhooks_responders(app_state.clone(), request, Bytes::new())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let requests = app_state
            .api
            .webhooks(&user)
            .get_responder_requests(responder.id)
            .await?;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].duration_ms.is_some());
        assert!(requests[0].response_status_code.is_none());
        assert!(requests[0].response_headers.is_none());
        assert!(requests[0].response_body.is_none());

        Ok(())
    }

    mod template_interpolation {
        use super::super::resolve_secret_templates;
        use std::collections::HashMap;

        fn make_secrets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        }

        #[test]
        fn resolves_single_reference() {
            let secrets = make_secrets(&[("API_KEY", "sk-123")]);
            assert_eq!(
                resolve_secret_templates("Bearer ${secrets.API_KEY}", &secrets),
                "Bearer sk-123"
            );
        }

        #[test]
        fn resolves_multiple_references() {
            let secrets = make_secrets(&[("A", "1"), ("B", "2")]);
            assert_eq!(
                resolve_secret_templates("${secrets.A}-${secrets.B}", &secrets),
                "1-2"
            );
        }

        #[test]
        fn leaves_unresolved_references() {
            let secrets = make_secrets(&[("A", "1")]);
            assert_eq!(
                resolve_secret_templates("${secrets.MISSING}", &secrets),
                "${secrets.MISSING}"
            );
        }

        #[test]
        fn no_references_returns_input() {
            let secrets = make_secrets(&[("A", "1")]);
            assert_eq!(
                resolve_secret_templates("no refs here", &secrets),
                "no refs here"
            );
        }

        #[test]
        fn handles_adjacent_references() {
            let secrets = make_secrets(&[("X", "a"), ("Y", "b")]);
            assert_eq!(
                resolve_secret_templates("${secrets.X}${secrets.Y}", &secrets),
                "ab"
            );
        }

        #[test]
        fn handles_empty_input() {
            let secrets = make_secrets(&[("A", "1")]);
            assert_eq!(resolve_secret_templates("", &secrets), "");
        }

        #[test]
        fn handles_partial_pattern() {
            let secrets = make_secrets(&[("A", "1")]);
            assert_eq!(
                resolve_secret_templates("${secrets.", &secrets),
                "${secrets."
            );
        }

        #[test]
        fn handles_value_containing_pattern_syntax() {
            let secrets = make_secrets(&[("A", "${secrets.B}")]);
            // The value itself looks like a pattern but shouldn't be re-expanded.
            let result = resolve_secret_templates("${secrets.A}", &secrets);
            assert_eq!(result, "${secrets.B}");
        }
    }
}
