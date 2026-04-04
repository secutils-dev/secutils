use actix_web::{
    Error,
    body::EitherBody,
    cookie::Cookie,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
};
use std::{
    future::{self, Future, Ready},
    pin::Pin,
    sync::Arc,
};

/// Middleware that appends an expired `Set-Cookie` header to every 401 response,
/// clearing the session cookie so the browser stops sending the stale value.
#[derive(Clone)]
pub struct ClearSessionCookie {
    cookie_name: Arc<str>,
}

impl ClearSessionCookie {
    pub fn new(cookie_name: impl Into<String>) -> Self {
        Self {
            cookie_name: Arc::from(cookie_name.into()),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for ClearSessionCookie
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ClearSessionCookieMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ready(Ok(ClearSessionCookieMiddleware {
            service,
            cookie_name: Arc::clone(&self.cookie_name),
        }))
    }
}

pub struct ClearSessionCookieMiddleware<S> {
    service: S,
    cookie_name: Arc<str>,
}

impl<S, B> Service<ServiceRequest> for ClearSessionCookieMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let cookie_name = Arc::clone(&self.cookie_name);
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            if res.status() == StatusCode::UNAUTHORIZED {
                let expired = Cookie::build(&*cookie_name, "")
                    .path("/")
                    .max_age(actix_web::cookie::time::Duration::ZERO)
                    .finish();
                let _ = res.response_mut().add_cookie(&expired);
            }
            Ok(res.map_into_left_body())
        })
    }
}
