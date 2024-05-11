use crate::{security::Credentials, server::app_state::AppState, users::User};
use actix_web::{
    dev::Payload,
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web, Error, FromRequest, HttpRequest,
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

impl FromRequest for User {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;

            let credentials = match Option::<BearerAuth>::extract(&req).await? {
                Some(bearer_auth) => Credentials::Jwt(bearer_auth.token().to_string()),
                None => Credentials::SessionCookie(
                    req.cookie(&state.config.security.session_cookie_name)
                        .ok_or_else(|| ErrorUnauthorized(anyhow!("Unauthorized")))?,
                ),
            };

            match state.api.security().authenticate(credentials).await {
                Ok(Some(user)) => Ok(user),
                Ok(None) => Err(ErrorUnauthorized(anyhow!("Unauthorized"))),
                Err(err) => {
                    log::error!("Failed to extract user information due to: {err:?}");
                    Err(ErrorInternalServerError(anyhow!("Internal server error")))
                }
            }
        })
    }
}
