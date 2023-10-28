use crate::{
    error::Error as SecutilsError,
    server::AppState,
    users::{User, UserShare},
    utils::UtilsLegacyAction,
};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BodyParams {
    action: UtilsLegacyAction,
}

pub async fn utils_handle_action(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    body_params: web::Json<BodyParams>,
) -> Result<HttpResponse, SecutilsError> {
    let action = body_params.into_inner().action;

    // Detect on behalf of what user to handle the action.
    let user = match (user, user_share) {
        // If user is authenticated, and action is not targeting a shared resource, act on behalf of
        // the currently authenticated user.
        (Some(user), None) => user,

        // If user is authenticated, and action is targeting a shared resource that belongs to the
        // user, act on behalf of the currently authenticated user.
        (Some(user), Some(user_share)) if user.id == user_share.user_id => user,

        // If action is targeting a shared resource that doesn't belong to currently authenticated
        // user or user isn't authenticated, act on behalf of the shared resource owner assuming
        // action is authorized to be performed on a shared resource.
        (_, Some(user_share)) if user_share.is_legacy_action_authorized(&action) => {
            // If user isn't found forbid any actions on the shared resource.
            if let Some(user) = state.api.users().get(user_share.user_id).await? {
                user
            } else {
                return Err(SecutilsError::access_forbidden());
            }
        }

        // Otherwise return "Access forbidden" error.
        _ => return Err(SecutilsError::access_forbidden()),
    };

    // Validate action parameters.
    if let Err(err) = action.validate(&state.api).await {
        log::error!(
            "User ({}) tried to perform invalid utility action: {err:?}",
            *user.id
        );
        return Err(err.into());
    }

    let user_id = user.id;
    match action.handle(user, &state.api).await {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(err) => {
            log::error!(
                "User ({}) failed to perform utility action: {err:?}",
                *user_id
            );
            Err(err.into())
        }
    }
}
