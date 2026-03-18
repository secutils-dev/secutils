use crate::{
    error::Error,
    server::app_state::AppState,
    users::{ScriptContext, User},
};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ScriptIdPath {
    pub script_id: Uuid,
}

#[derive(Deserialize)]
pub struct ListScriptsQuery {
    pub context: Option<ScriptContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScriptBody {
    pub name: String,
    pub script_type: String,
    pub content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScriptBody {
    pub content: String,
}

/// GET /api/user/scripts
pub async fn user_scripts_list(
    state: web::Data<AppState>,
    user: User,
    query: web::Query<ListScriptsQuery>,
) -> Result<HttpResponse, Error> {
    let scripts = state.api.scripts(&user).list_scripts(query.context).await?;
    Ok(HttpResponse::Ok().json(scripts))
}

/// GET /api/user/scripts/{script_id}
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

/// POST /api/user/scripts
pub async fn user_scripts_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<CreateScriptBody>,
) -> Result<HttpResponse, Error> {
    let script = state
        .api
        .scripts(&user)
        .create_script(&body.name, &body.script_type, &body.content)
        .await?;
    Ok(HttpResponse::Created().json(script))
}

/// PUT /api/user/scripts/{script_id}
pub async fn user_scripts_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ScriptIdPath>,
    body: web::Json<UpdateScriptBody>,
) -> Result<HttpResponse, Error> {
    let script = state
        .api
        .scripts(&user)
        .update_script(path.script_id, &body.content)
        .await?;
    Ok(HttpResponse::Ok().json(script))
}

/// DELETE /api/user/scripts/{script_id}
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
