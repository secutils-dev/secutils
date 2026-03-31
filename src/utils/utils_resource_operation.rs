use crate::utils::UtilsResource;
use actix_web::http::Method;

/// Describe custom util's resource operation.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResourceOperation {
    WebScrapingPageGetHistory,
    WebScrapingPageClearHistory,
    WebScrapingPageGetLogs,
    WebScrapingPageClearLogs,
    WebScrapingPageGetLogsSummary,
    WebScrapingPageDebugRequest,
    WebScrapingApiGetHistory,
    WebScrapingApiClearHistory,
    WebScrapingApiGetLogs,
    WebScrapingApiClearLogs,
    WebScrapingApiGetLogsSummary,
    WebScrapingApiTestRequest,
    WebScrapingApiDebugRequest,
}

impl UtilsResourceOperation {
    /// Returns true if the operation requires parameters (via HTTP body).
    pub fn requires_params(&self) -> bool {
        matches!(
            self,
            Self::WebScrapingPageGetHistory
                | Self::WebScrapingPageDebugRequest
                | Self::WebScrapingApiGetHistory
                | Self::WebScrapingApiTestRequest
                | Self::WebScrapingApiDebugRequest
        )
    }
}

impl TryFrom<(&UtilsResource, &str, &Method)> for UtilsResourceOperation {
    type Error = ();

    fn try_from(
        (resource, operation, method): (&UtilsResource, &str, &Method),
    ) -> Result<Self, Self::Error> {
        match resource {
            // Web scraping custom actions.
            UtilsResource::WebScrapingPage if operation == "history" => {
                Ok(UtilsResourceOperation::WebScrapingPageGetHistory)
            }
            UtilsResource::WebScrapingPage if operation == "clear" => {
                Ok(UtilsResourceOperation::WebScrapingPageClearHistory)
            }
            UtilsResource::WebScrapingPage if operation == "logs" && *method == Method::GET => {
                Ok(UtilsResourceOperation::WebScrapingPageGetLogs)
            }
            UtilsResource::WebScrapingPage if operation == "clear_logs" => {
                Ok(UtilsResourceOperation::WebScrapingPageClearLogs)
            }
            UtilsResource::WebScrapingPage
                if operation == "logs_summary" && *method == Method::GET =>
            {
                Ok(UtilsResourceOperation::WebScrapingPageGetLogsSummary)
            }
            UtilsResource::WebScrapingPage if operation == "debug" && *method == Method::POST => {
                Ok(UtilsResourceOperation::WebScrapingPageDebugRequest)
            }
            UtilsResource::WebScrapingApi if operation == "history" => {
                Ok(UtilsResourceOperation::WebScrapingApiGetHistory)
            }
            UtilsResource::WebScrapingApi if operation == "clear" => {
                Ok(UtilsResourceOperation::WebScrapingApiClearHistory)
            }
            UtilsResource::WebScrapingApi if operation == "logs" && *method == Method::GET => {
                Ok(UtilsResourceOperation::WebScrapingApiGetLogs)
            }
            UtilsResource::WebScrapingApi if operation == "clear_logs" => {
                Ok(UtilsResourceOperation::WebScrapingApiClearLogs)
            }
            UtilsResource::WebScrapingApi
                if operation == "logs_summary" && *method == Method::GET =>
            {
                Ok(UtilsResourceOperation::WebScrapingApiGetLogsSummary)
            }
            UtilsResource::WebScrapingApi if operation == "test" && *method == Method::POST => {
                Ok(UtilsResourceOperation::WebScrapingApiTestRequest)
            }
            UtilsResource::WebScrapingApi if operation == "debug" && *method == Method::POST => {
                Ok(UtilsResourceOperation::WebScrapingApiDebugRequest)
            }

            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsResourceOperation;
    use crate::utils::UtilsResource;
    use actix_web::http::Method;

    #[test]
    fn properly_checks_if_action_requires_params() {
        assert!(UtilsResourceOperation::WebScrapingPageGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingPageClearHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingPageGetLogs.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingPageClearLogs.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingPageGetLogsSummary.requires_params());
        assert!(UtilsResourceOperation::WebScrapingPageDebugRequest.requires_params());

        assert!(UtilsResourceOperation::WebScrapingApiGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingApiClearHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingApiGetLogs.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingApiClearLogs.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingApiGetLogsSummary.requires_params());
        assert!(UtilsResourceOperation::WebScrapingApiTestRequest.requires_params());
        assert!(UtilsResourceOperation::WebScrapingApiDebugRequest.requires_params());
    }

    #[test]
    fn properly_parses_resource_action_operation() {
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageClearHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "logs",
                &Method::GET
            )),
            Ok(UtilsResourceOperation::WebScrapingPageGetLogs)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "clear_logs",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageClearLogs)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "logs_summary",
                &Method::GET
            )),
            Ok(UtilsResourceOperation::WebScrapingPageGetLogsSummary)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "debug",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageDebugRequest)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "debug",
                &Method::GET
            ))
            .is_err()
        );

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingApiGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingApiClearHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "logs",
                &Method::GET
            )),
            Ok(UtilsResourceOperation::WebScrapingApiGetLogs)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "clear_logs",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingApiClearLogs)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "logs_summary",
                &Method::GET
            )),
            Ok(UtilsResourceOperation::WebScrapingApiGetLogsSummary)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "test",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingApiTestRequest)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "test",
                &Method::GET
            ))
            .is_err()
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "debug",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingApiDebugRequest)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingApi,
                "debug",
                &Method::GET
            ))
            .is_err()
        );
    }
}
