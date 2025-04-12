use crate::{security::Credentials, server::app_state::AppState};
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error::ErrorUnauthorized, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

impl FromRequest for Credentials {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;
            Ok(match Option::<BearerAuth>::extract(&req).await? {
                Some(bearer_auth) => Credentials::Jwt(bearer_auth.token().to_string()),
                None => Credentials::SessionCookie(
                    req.cookie(&state.config.security.session_cookie_name)
                        .ok_or_else(|| ErrorUnauthorized(anyhow!("Unauthorized")))?,
                ),
            })
        })
    }
}
