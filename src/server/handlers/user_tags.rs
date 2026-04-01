use crate::{
    error::Error,
    server::app_state::AppState,
    users::{TagCreateParams, TagUpdateParams, User, UserTag},
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
pub struct TagIdPath {
    pub tag_id: Uuid,
}

/// Lists all tags for the authenticated user.
#[utoipa::path(
    tags = ["tags"],
    responses(
        (status = 200, description = "List of user tags.", body = [UserTag]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/user/tags")]
pub async fn user_tags_list(state: web::Data<AppState>, user: User) -> Result<HttpResponse, Error> {
    let tags = state.api.tags(&user).list_tags().await?;
    Ok(HttpResponse::Ok().json(tags))
}

/// Creates a new tag.
#[utoipa::path(
    tags = ["tags"],
    request_body = TagCreateParams,
    responses(
        (status = 201, description = "Tag was successfully created.", body = UserTag),
        (status = BAD_REQUEST, description = "Invalid tag parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/user/tags")]
pub async fn user_tags_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<TagCreateParams>,
) -> Result<HttpResponse, Error> {
    let tag = state.api.tags(&user).create_tag(body.into_inner()).await?;
    Ok(HttpResponse::Created().json(tag))
}

/// Updates an existing tag's name and/or color.
#[utoipa::path(
    tags = ["tags"],
    params(TagIdPath),
    request_body = TagUpdateParams,
    responses(
        (status = 200, description = "Tag was successfully updated.", body = UserTag),
        (status = NOT_FOUND, description = "Tag not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/user/tags/{tag_id}")]
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

/// Deletes a tag by ID.
#[utoipa::path(
    tags = ["tags"],
    params(TagIdPath),
    responses(
        (status = 204, description = "Tag was successfully deleted."),
        (status = NOT_FOUND, description = "Tag not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/user/tags/{tag_id}")]
pub async fn user_tags_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TagIdPath>,
) -> Result<HttpResponse, Error> {
    state.api.tags(&user).delete_tag(path.tag_id).await?;
    Ok(HttpResponse::NoContent().finish())
}
