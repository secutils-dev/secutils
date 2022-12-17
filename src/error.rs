use actix_web::{
    http::{header, StatusCode},
    HttpResponse, HttpResponseBuilder, ResponseError,
};
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub struct SecutilsError {
    err: anyhow::Error,
}

impl Display for SecutilsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl ResponseError for SecutilsError {
    fn error_response(&self) -> HttpResponse {
        log::error!("Response error: {} {}", self.status_code(), self.err);
        HttpResponseBuilder::new(self.status_code())
            .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
            .body(match self.status_code() {
                StatusCode::UNAUTHORIZED => "Unauthorized",
                _ => "Internal Server Error",
            })
    }
}

impl From<anyhow::Error> for SecutilsError {
    fn from(err: anyhow::Error) -> SecutilsError {
        SecutilsError { err }
    }
}
