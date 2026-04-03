use super::resolve_shared_user;
use crate::{
    error::Error,
    server::app_state::AppState,
    users::{ClientUserShare, SharedResource, User, UserShare},
    utils::web_security::{
        ContentSecurityPoliciesCreateParams, ContentSecurityPoliciesSerializeParams,
        ContentSecurityPoliciesUpdateParams, ContentSecurityPolicy,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Serialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct PolicyIdPath {
    pub policy_id: Uuid,
}

/// Response for GET /api/web_security/csp/{policy_id}.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentSecurityPolicyGetResponse {
    pub policy: ContentSecurityPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_share: Option<ClientUserShare>,
}

/// Lists all content security policies for the authenticated user.
#[utoipa::path(
    tags = ["web_security"],
    responses(
        (status = 200, description = "List of content security policies.", body = [ContentSecurityPolicy]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_security/csp")]
pub async fn csp_list(state: web::Data<AppState>, user: User) -> Result<HttpResponse, Error> {
    let policies = state
        .api
        .web_security()
        .get_content_security_policies(user.id)
        .await?;
    Ok(HttpResponse::Ok().json(policies))
}

/// Gets a content security policy by ID, including its share status.
///
/// Supports shared access: when an `X-User-Share-ID` header is present and points to
/// a share for this policy, the request is served on behalf of the share owner - even
/// if the caller is unauthenticated.
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    security((), ("bearerAuth" = [])),
    responses(
        (status = 200, description = "Content security policy with share info.", body = ContentSecurityPolicyGetResponse),
        (status = 404, description = "Policy not found.")
    )
)]
#[get("/api/web_security/csp/{policy_id}")]
pub async fn csp_get(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    path: web::Path<PolicyIdPath>,
) -> Result<HttpResponse, Error> {
    let user = resolve_shared_user(
        &state,
        user,
        user_share,
        &SharedResource::content_security_policy(path.policy_id),
    )
    .await?;

    let web_security = state.api.web_security();
    let Some(policy) = web_security
        .get_content_security_policy(user.id, path.policy_id)
        .await?
    else {
        return Err(Error::not_found("Content security policy not found."));
    };

    let user_share = state
        .api
        .users()
        .get_user_share_by_resource(
            user.id,
            &SharedResource::content_security_policy(path.policy_id),
        )
        .await?
        .map(ClientUserShare::from);

    Ok(HttpResponse::Ok().json(ContentSecurityPolicyGetResponse { policy, user_share }))
}

/// Creates a new content security policy.
#[utoipa::path(
    tags = ["web_security"],
    request_body = ContentSecurityPoliciesCreateParams,
    responses(
        (status = 201, description = "Policy was successfully created.", body = ContentSecurityPolicy),
        (status = BAD_REQUEST, description = "Invalid policy parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_security/csp")]
pub async fn csp_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<ContentSecurityPoliciesCreateParams>,
) -> Result<HttpResponse, Error> {
    let policy = state
        .api
        .web_security()
        .create_content_security_policy(user.id, body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(policy))
}

/// Updates an existing content security policy.
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    request_body = ContentSecurityPoliciesUpdateParams,
    responses(
        (status = 204, description = "Policy was successfully updated."),
        (status = NOT_FOUND, description = "Policy not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/web_security/csp/{policy_id}")]
pub async fn csp_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<PolicyIdPath>,
    body: web::Json<ContentSecurityPoliciesUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_security()
        .update_content_security_policy(user.id, path.policy_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a content security policy by ID.
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    responses(
        (status = 204, description = "Policy was successfully deleted."),
        (status = NOT_FOUND, description = "Policy not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/web_security/csp/{policy_id}")]
pub async fn csp_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<PolicyIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_security()
        .remove_content_security_policy(user.id, path.policy_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Serializes a content security policy into a header string.
///
/// Supports shared access (see `csp_get`).
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    request_body = ContentSecurityPoliciesSerializeParams,
    security((), ("bearerAuth" = [])),
    responses(
        (status = 200, description = "Serialized CSP header string.", body = String),
        (status = NOT_FOUND, description = "Policy not found.")
    )
)]
#[post("/api/web_security/csp/{policy_id}/_serialize")]
pub async fn csp_serialize(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    path: web::Path<PolicyIdPath>,
    body: web::Json<ContentSecurityPoliciesSerializeParams>,
) -> Result<HttpResponse, Error> {
    let user = resolve_shared_user(
        &state,
        user,
        user_share,
        &SharedResource::content_security_policy(path.policy_id),
    )
    .await?;

    let data = state
        .api
        .web_security()
        .serialize_content_security_policy(user.id, path.policy_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// Shares a content security policy, making it accessible via a share link.
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    responses(
        (status = 200, description = "Share info for the policy."),
        (status = NOT_FOUND, description = "Policy not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_security/csp/{policy_id}/_share")]
pub async fn csp_share(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<PolicyIdPath>,
) -> Result<HttpResponse, Error> {
    let user_share = state
        .api
        .web_security()
        .share_content_security_policy(user.id, path.policy_id)
        .await
        .map(ClientUserShare::from)?;
    Ok(HttpResponse::Ok().json(user_share))
}

/// Removes sharing from a content security policy.
#[utoipa::path(
    tags = ["web_security"],
    params(PolicyIdPath),
    responses(
        (status = 204, description = "Policy was successfully unshared."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_security/csp/{policy_id}/_unshare")]
pub async fn csp_unshare(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<PolicyIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_security()
        .unshare_content_security_policy(user.id, path.policy_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::web_security::{
            ContentSecurityPoliciesCreateParams, ContentSecurityPoliciesSerializeParams,
            ContentSecurityPoliciesUpdateParams,
        },
    };

    #[test]
    fn csp_create_params_example_is_valid() {
        let example: ContentSecurityPoliciesCreateParams =
            serde_json::from_value(schema_example::<ContentSecurityPoliciesCreateParams>())
                .unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn csp_update_params_example_is_valid() {
        let example: ContentSecurityPoliciesUpdateParams =
            serde_json::from_value(schema_example::<ContentSecurityPoliciesUpdateParams>())
                .unwrap();
        assert!(
            example.name.is_some() || example.directives.is_some() || example.tag_ids.is_some()
        );
    }

    #[test]
    fn csp_serialize_params_example_is_valid() {
        let _: ContentSecurityPoliciesSerializeParams =
            serde_json::from_value(schema_example::<ContentSecurityPoliciesSerializeParams>())
                .unwrap();
    }
}
