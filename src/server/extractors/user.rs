use crate::{server::app_state::AppState, users::User};
use actix_http::Payload;
use actix_identity::Identity;
use actix_web::{error::ErrorUnauthorized, web, Error, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::basic::BasicAuth;
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

impl FromRequest for User {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;

            // First check basic auth, don't fallback to the session even if basic auth isn't valid.
            let basic_auth = Option::<BasicAuth>::extract(&req).await?;
            if let Some(basic_auth) = basic_auth {
                if let Some(password) = basic_auth.password() {
                    return match state
                        .api
                        .users()
                        .authenticate(basic_auth.user_id(), password)
                        .await
                    {
                        Ok(user) => Ok(user),
                        Err(err) => {
                            log::error!("{}", err);
                            return Err(ErrorUnauthorized(anyhow!("Unauthorized")));
                        }
                    };
                }

                return Err(ErrorUnauthorized(anyhow!("Unauthorized")));
            }

            let identity = Option::<Identity>::extract(&req).await?;
            if let Some(identity_id) = identity.and_then(|identity| identity.id().ok()) {
                return state
                    .api
                    .users()
                    .get_by_email(identity_id)
                    .await
                    .and_then(|user| {
                        if let Some(user) = user {
                            Ok(user)
                        } else {
                            Err(anyhow!("Unauthorized"))
                        }
                    })
                    .map_err(|_| ErrorUnauthorized(anyhow!("Unauthorized")));
            }

            Err(ErrorUnauthorized(anyhow!("Unauthorized")))
        })
    }
}
