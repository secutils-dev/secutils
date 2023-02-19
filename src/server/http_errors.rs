use actix_web::HttpResponse;
use serde_json::json;

pub fn generic_internal_server_error() -> HttpResponse {
    HttpResponse::InternalServerError()
        .json(json!({ "message": "The operation could not be completed due to a system error. Please try again later or contact us for assistance." }))
}

#[cfg(test)]
mod tests {
    use crate::server::http_errors::generic_internal_server_error;

    #[test]
    fn creates_generic_internal_server_error() -> anyhow::Result<()> {
        let response = generic_internal_server_error();
        assert_eq!(response.status().as_u16(), 500);

        Ok(())
    }
}
