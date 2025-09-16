use crate::{
    security::{Credentials, Operator},
    server::app_state::AppState,
};
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error::ErrorUnauthorized, web};
use anyhow::anyhow;
use std::{future::Future, pin::Pin};
use tracing::{error, warn};

impl FromRequest for Operator {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;
            let credentials = Credentials::extract(&req).await?;
            match state.api.security().get_operator(&credentials).await {
                Ok(Some(user)) => Ok(user),
                Ok(None) => {
                    warn!(
                        request_path = req.path(),
                        "Non-operator tried to access protected endpoint."
                    );
                    Err(ErrorUnauthorized(anyhow!("Unauthorized")))
                }
                Err(err) => {
                    error!(
                        request_path = req.path(),
                        "Failed to extract operator information due to: {err:?}"
                    );
                    Err(ErrorUnauthorized(anyhow!("Unauthorized")))
                }
            }
        })
    }
}
