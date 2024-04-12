use crate::{server::app_state::AppState, users::User};
use actix_web::{
    dev::Payload,
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web, Error, FromRequest, HttpRequest,
};
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

impl FromRequest for User {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;

            // Check if request has a session cookie.
            let Some(cookie) = req.cookie(&state.config.security.session_cookie_name) else {
                return Err(ErrorUnauthorized(anyhow!("Unauthorized")));
            };

            let cookie_string = format!("{}={}", cookie.name(), cookie.value());
            match state.api.security().authenticate(cookie_string).await {
                Ok(Some(user)) => Ok(user),
                Ok(None) => Err(ErrorUnauthorized(anyhow!("Unauthorized"))),
                Err(_) => Err(ErrorInternalServerError(anyhow!("Internal server error"))),
            }
        })
    }
}
