use crate::{
    error::Error,
    server::app_state::AppState,
    users::{TagCreateParams, TagUpdateParams, User},
};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TagIdPath {
    pub tag_id: Uuid,
}

/// GET /api/user/tags
pub async fn user_tags_list(state: web::Data<AppState>, user: User) -> Result<HttpResponse, Error> {
    let tags = state.api.tags(&user).list_tags().await?;
    Ok(HttpResponse::Ok().json(tags))
}

/// POST /api/user/tags
pub async fn user_tags_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<TagCreateParams>,
) -> Result<HttpResponse, Error> {
    let tag = state.api.tags(&user).create_tag(body.into_inner()).await?;
    Ok(HttpResponse::Created().json(tag))
}

/// PUT /api/user/tags/{tag_id}
pub async fn user_tags_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TagIdPath>,
    body: web::Json<TagUpdateParams>,
) -> Result<HttpResponse, Error> {
    let tag = state
        .api
        .tags(&user)
        .update_tag(path.tag_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(tag))
}

/// DELETE /api/user/tags/{tag_id}
pub async fn user_tags_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TagIdPath>,
) -> Result<HttpResponse, Error> {
    state.api.tags(&user).delete_tag(path.tag_id).await?;
    Ok(HttpResponse::NoContent().finish())
}
