use crate::{
    security::{Credentials, Operator},
    server::app_state::AppState,
};
use actix_web::{
    dev::Payload,
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web, Error, FromRequest, HttpRequest,
};
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

impl FromRequest for Operator {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let state = web::Data::<AppState>::extract(&req).await?;
            let credentials = Credentials::extract(&req).await?;
            match state.api.security().get_operator(credentials).await {
                Ok(Some(user)) => Ok(user),
                Ok(None) => {
                    log::warn!(request_path:serde = req.path(); "Non-operator tried to access protected endpoint.");
                    Err(ErrorUnauthorized(anyhow!("Unauthorized")))
                }
                Err(err) => {
                    log::error!(request_path:serde = req.path(); "Failed to extract operator information due to: {err:?}");
                    Err(ErrorInternalServerError(anyhow!("Internal server error")))
                }
            }
        })
    }
}
