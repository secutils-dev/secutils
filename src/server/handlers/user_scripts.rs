use crate::{
    error::Error,
    server::app_state::AppState,
    users::{ScriptContext, ScriptCreateParams, ScriptUpdateParams, User, UserScript},
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
pub struct ScriptIdPath {
    pub script_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
pub struct ListScriptsQuery {
    /// Optional context to filter scripts by compatibility.
    pub context: Option<ScriptContext>,
}

/// Lists all scripts for the authenticated user, optionally filtered by context.
#[utoipa::path(
    tags = ["scripts"],
    params(ListScriptsQuery),
    responses(
        (status = 200, description = "List of user scripts.", body = [UserScript])
    )
)]
#[get("/api/user/scripts")]
pub async fn user_scripts_list(
    state: web::Data<AppState>,
    user: User,
    query: web::Query<ListScriptsQuery>,
) -> Result<HttpResponse, Error> {
    let scripts = state.api.scripts(&user).list_scripts(query.context).await?;
    Ok(HttpResponse::Ok().json(scripts))
}

/// Gets a single script by ID, including its content.
#[utoipa::path(
    tags = ["scripts"],
    params(ScriptIdPath),
    responses(
        (status = 200, description = "Script with the specified ID.", body = UserScript),
        (status = NOT_FOUND, description = "Script not found.")
    )
)]
#[get("/api/user/scripts/{script_id}")]
pub async fn user_scripts_get(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ScriptIdPath>,
) -> Result<HttpResponse, Error> {
    match state.api.scripts(&user).get_script(path.script_id).await? {
        Some(script) => Ok(HttpResponse::Ok().json(script)),
        None => Err(Error::not_found("Script not found.")),
    }
}

/// Creates a new script.
#[utoipa::path(
    tags = ["scripts"],
    request_body = ScriptCreateParams,
    responses(
        (status = 201, description = "Script was successfully created.", body = UserScript),
        (status = BAD_REQUEST, description = "Invalid script parameters.")
    )
)]
#[post("/api/user/scripts")]
pub async fn user_scripts_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<ScriptCreateParams>,
) -> Result<HttpResponse, Error> {
    let script = state
        .api
        .scripts(&user)
        .create_script(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(script))
}

/// Updates an existing script's content.
#[utoipa::path(
    tags = ["scripts"],
    params(ScriptIdPath),
    request_body = ScriptUpdateParams,
    responses(
        (status = 200, description = "Script was successfully updated.", body = UserScript),
        (status = NOT_FOUND, description = "Script not found.")
    )
)]
#[put("/api/user/scripts/{script_id}")]
pub async fn user_scripts_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ScriptIdPath>,
    body: web::Json<ScriptUpdateParams>,
) -> Result<HttpResponse, Error> {
    let script = state
        .api
        .scripts(&user)
        .update_script(path.script_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(script))
}

/// Deletes a script by ID.
#[utoipa::path(
    tags = ["scripts"],
    params(ScriptIdPath),
    responses(
        (status = 204, description = "Script was successfully deleted."),
        (status = NOT_FOUND, description = "Script not found.")
    )
)]
#[delete("/api/user/scripts/{script_id}")]
pub async fn user_scripts_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ScriptIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .scripts(&user)
        .delete_script(path.script_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
