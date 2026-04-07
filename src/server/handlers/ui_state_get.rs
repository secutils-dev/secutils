use crate::{
    error::Error as SecutilsError,
    server::{AppState, SubscriptionState, UiPlatformState, UiState},
    users::{ClientUserShare, User, UserShare},
};
use actix_web::{HttpResponse, web};
use anyhow::anyhow;
use std::ops::Deref;
use tracing::error;

pub async fn ui_state_get(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
) -> Result<HttpResponse, SecutilsError> {
    // Settings only available for authenticated users.
    let settings = if let Some(ref user) = user {
        state.api.settings(user).get_settings().await?
    } else {
        None
    };

    // Utils are only available for authenticated users or when accessing shared resources.
    let utils = if user.is_some() || user_share.is_some() {
        state.api.utils().get_all().await?
    } else {
        vec![]
    };

    let status = state.status.read().map_err(|err| {
        error!("Failed to read status: {err}");
        SecutilsError::from(anyhow!("Status is not available."))
    })?;

    let subscription = user.as_ref().map(|user| SubscriptionState {
        features: user.subscription.get_features(&state.config).into(),
        manage_url: state.config.subscriptions.manage_url.as_ref(),
        feature_overview_url: state.config.subscriptions.feature_overview_url.as_ref(),
    });

    Ok(HttpResponse::Ok().json(UiState {
        status: status.deref(),
        user,
        subscription,
        user_share: user_share.map(ClientUserShare::from),
        settings,
        utils,
        platform: UiPlatformState {
            max_import_file_size: state.config.platform.max_import_file_size,
        },
    }))
}
