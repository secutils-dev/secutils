use super::resolve_shared_user;
use crate::{
    error::Error,
    server::app_state::AppState,
    users::{ClientUserShare, SharedResource, User, UserShare},
    utils::certificates::{
        CertificateTemplate, TemplatesCreateParams, TemplatesFetchCertificatesParams,
        TemplatesGenerateParams, TemplatesUpdateParams,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Serialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct TemplateIdPath {
    pub template_id: Uuid,
}

/// Response for GET /api/certificates/templates/{template_id}.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplateGetResponse {
    pub template: CertificateTemplate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_share: Option<ClientUserShare>,
}

/// Lists all certificate templates for the authenticated user.
#[utoipa::path(
    tags = ["certificates"],
    responses(
        (status = 200, description = "List of certificate templates.", body = [CertificateTemplate]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/certificates/templates")]
pub async fn certificate_templates_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let templates = state
        .api
        .certificates()
        .get_certificate_templates(user.id)
        .await?;
    Ok(HttpResponse::Ok().json(templates))
}

/// Gets a certificate template by ID, including its share status.
///
/// Supports shared access: when an `X-User-Share-ID` header is present and points to
/// a share for this template, the request is served on behalf of the share owner - even
/// if the caller is unauthenticated.
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    security((), ("bearerAuth" = [])),
    responses(
        (status = 200, description = "Certificate template with share info.", body = CertificateTemplateGetResponse),
        (status = 404, description = "Template not found.")
    )
)]
#[get("/api/certificates/templates/{template_id}")]
pub async fn certificate_templates_get(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    path: web::Path<TemplateIdPath>,
) -> Result<HttpResponse, Error> {
    let user = resolve_shared_user(
        &state,
        user,
        user_share,
        &SharedResource::certificate_template(path.template_id),
    )
    .await?;

    let certificates = state.api.certificates();
    let Some(template) = certificates
        .get_certificate_template(user.id, path.template_id)
        .await?
    else {
        return Err(Error::not_found("Certificate template not found."));
    };

    let user_share = state
        .api
        .users()
        .get_user_share_by_resource(
            user.id,
            &SharedResource::certificate_template(path.template_id),
        )
        .await?
        .map(ClientUserShare::from);

    Ok(HttpResponse::Ok().json(CertificateTemplateGetResponse {
        template,
        user_share,
    }))
}

/// Creates a new certificate template.
#[utoipa::path(
    tags = ["certificates"],
    request_body = TemplatesCreateParams,
    responses(
        (status = 201, description = "Template was successfully created.", body = CertificateTemplate),
        (status = BAD_REQUEST, description = "Invalid template parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/templates")]
pub async fn certificate_templates_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<TemplatesCreateParams>,
) -> Result<HttpResponse, Error> {
    let template = state
        .api
        .certificates()
        .create_certificate_template(user.id, body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(template))
}

/// Updates an existing certificate template.
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    request_body = TemplatesUpdateParams,
    responses(
        (status = 204, description = "Template was successfully updated."),
        (status = NOT_FOUND, description = "Template not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/certificates/templates/{template_id}")]
pub async fn certificate_templates_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TemplateIdPath>,
    body: web::Json<TemplatesUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .certificates()
        .update_certificate_template(user.id, path.template_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a certificate template by ID.
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    responses(
        (status = 204, description = "Template was successfully deleted."),
        (status = NOT_FOUND, description = "Template not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/certificates/templates/{template_id}")]
pub async fn certificate_templates_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TemplateIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .certificates()
        .remove_certificate_template(user.id, path.template_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Generates a self-signed certificate from the template.
///
/// Supports shared access (see `certificate_templates_get`).
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    request_body = TemplatesGenerateParams,
    security((), ("bearerAuth" = [])),
    responses(
        (status = 200, description = "Generated certificate data (binary, base64-encoded in JSON)."),
        (status = NOT_FOUND, description = "Template not found.")
    )
)]
#[post("/api/certificates/templates/{template_id}/_generate")]
pub async fn certificate_templates_generate(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    path: web::Path<TemplateIdPath>,
    body: web::Json<TemplatesGenerateParams>,
) -> Result<HttpResponse, Error> {
    let user = resolve_shared_user(
        &state,
        user,
        user_share,
        &SharedResource::certificate_template(path.template_id),
    )
    .await?;

    let data = state
        .api
        .certificates()
        .generate_self_signed_certificate(user.id, path.template_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// Shares a certificate template, making it accessible via a share link.
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    responses(
        (status = 200, description = "Share info for the template."),
        (status = NOT_FOUND, description = "Template not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/templates/{template_id}/_share")]
pub async fn certificate_templates_share(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TemplateIdPath>,
) -> Result<HttpResponse, Error> {
    let user_share = state
        .api
        .certificates()
        .share_certificate_template(user.id, path.template_id)
        .await
        .map(ClientUserShare::from)?;
    Ok(HttpResponse::Ok().json(user_share))
}

/// Removes sharing from a certificate template.
#[utoipa::path(
    tags = ["certificates"],
    params(TemplateIdPath),
    responses(
        (status = 204, description = "Template was successfully unshared."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/templates/{template_id}/_unshare")]
pub async fn certificate_templates_unshare(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TemplateIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .certificates()
        .unshare_certificate_template(user.id, path.template_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Fetches the TLS certificate chain from a remote HTTPS endpoint.
#[utoipa::path(
    tags = ["certificates"],
    request_body = TemplatesFetchCertificatesParams,
    responses(
        (status = 200, description = "PEM-encoded certificate chain.", body = [String]),
        (status = BAD_REQUEST, description = "Invalid or unreachable URL."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/_fetch")]
pub async fn certificates_fetch(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<TemplatesFetchCertificatesParams>,
) -> Result<HttpResponse, Error> {
    let _ = user; // authenticated but not used for this operation
    let certs = state
        .api
        .certificates()
        .get_peer_certificates(&body.url)
        .await?;
    Ok(HttpResponse::Ok().json(certs))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::certificates::{
            TemplatesCreateParams, TemplatesFetchCertificatesParams, TemplatesGenerateParams,
            TemplatesUpdateParams,
        },
    };

    #[test]
    fn templates_create_params_example_is_valid() {
        let example: TemplatesCreateParams =
            serde_json::from_value(schema_example::<TemplatesCreateParams>()).unwrap();
        assert!(!example.template_name.is_empty());
    }

    #[test]
    fn templates_update_params_example_is_valid() {
        let example: TemplatesUpdateParams =
            serde_json::from_value(schema_example::<TemplatesUpdateParams>()).unwrap();
        assert!(
            example.template_name.is_some()
                || example.attributes.is_some()
                || example.tag_ids.is_some()
        );
    }

    #[test]
    fn templates_generate_params_example_is_valid() {
        let _: TemplatesGenerateParams =
            serde_json::from_value(schema_example::<TemplatesGenerateParams>()).unwrap();
    }

    #[test]
    fn templates_peer_certificates_params_example_is_valid() {
        let example: TemplatesFetchCertificatesParams =
            serde_json::from_value(schema_example::<TemplatesFetchCertificatesParams>()).unwrap();
        assert_eq!(example.url.scheme(), "https");
    }
}
