use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    server::AppState,
    users::{User, UserShare},
    utils::{
        UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
        certificates::certificates_handle_action, web_scraping::web_scraping_handle_action,
        web_security::web_security_handle_action, webhooks::webhooks_handle_action,
    },
};
use actix_web::{HttpRequest, HttpResponse, http::Method, web};
use serde_json::Value as JsonValue;
use tracing::error;
use uuid::Uuid;

fn extract_resource(req: &HttpRequest) -> Option<UtilsResource> {
    let match_info = req.match_info();
    let (Some(area), Some(resource)) = (match_info.get("area"), match_info.get("resource")) else {
        return None;
    };

    UtilsResource::try_from((area, resource)).ok()
}

fn extract_action(req: &HttpRequest, resource: &UtilsResource) -> Option<UtilsAction> {
    let match_info = req.match_info();
    let (generic_resource_operation, custom_resource_operation) = (
        match_info.get("resource_operation"),
        match_info
            .get("resource_operation")
            .map(|operation| UtilsResourceOperation::try_from((resource, operation, req.method())))
            .transpose(),
    );

    // If resource id cannot be parsed, and `resource_operation` parameter isn't provided, treat
    // `resource_id` as a custom **global** resource operation.
    let resource_id = match_info
        .get("resource_id")
        .map(Uuid::parse_str)
        .transpose();
    let (Ok(resource_id), custom_resource_operation) = (match resource_id {
        Err(_) if generic_resource_operation.is_none() => (
            Ok(None),
            match_info
                .get("resource_id")
                .map(|operation| {
                    UtilsResourceOperation::try_from((resource, operation, req.method()))
                })
                .transpose(),
        ),
        _ => (resource_id, custom_resource_operation),
    }) else {
        return None;
    };

    match (
        req.method(),
        resource_id,
        custom_resource_operation,
        generic_resource_operation,
    ) {
        // Resource-collection-based actions.
        (&Method::GET, None, Ok(None), None) => Some(UtilsAction::List),
        (&Method::POST, None, Ok(None), None) => Some(UtilsAction::Create),
        // Resource based actions.
        (&Method::GET, Some(resource_id), Ok(None), None) => Some(UtilsAction::Get { resource_id }),
        (&Method::PUT, Some(resource_id), Ok(None), None) => {
            Some(UtilsAction::Update { resource_id })
        }
        (&Method::DELETE, Some(resource_id), Ok(None), None) => {
            Some(UtilsAction::Delete { resource_id })
        }
        (&Method::POST, Some(resource_id), _, Some("share")) => {
            Some(UtilsAction::Share { resource_id })
        }
        (&Method::POST, Some(resource_id), _, Some("unshare")) => {
            Some(UtilsAction::Unshare { resource_id })
        }
        (&Method::GET | &Method::POST, resource_id, Ok(Some(operation)), _) => {
            Some(UtilsAction::Execute {
                resource_id,
                operation,
            })
        }
        // Unsupported actions.
        _ => None,
    }
}

async fn extract_user<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: Option<User>,
    user_share: Option<UserShare>,
    action: &UtilsAction,
    resource: &UtilsResource,
) -> anyhow::Result<Option<User>> {
    match (user, user_share) {
        // If user is authenticated, and action is not targeting a shared resource, act on behalf of
        // the currently authenticated user.
        (user, None) if user.is_some() => Ok(user),

        // If user is authenticated, and action is targeting a shared resource that belongs to the
        // user, act on behalf of the currently authenticated user.
        (Some(user), Some(user_share)) if user.id == user_share.user_id => Ok(Some(user)),

        // If action is targeting a shared resource that doesn't belong to currently authenticated
        // user or user isn't authenticated, act on behalf of the shared resource owner assuming
        // action is authorized to be performed on a shared resource.
        (_, Some(user_share)) if user_share.is_action_authorized(action, resource) => {
            api.users().get(user_share.user_id).await
        }

        _ => Ok(None),
    }
}

pub async fn utils_action(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    req: HttpRequest,
    body_params: Option<web::Json<JsonValue>>,
) -> Result<HttpResponse, SecutilsError> {
    // First, extract resource.
    let Some(resource) = extract_resource(&req) else {
        return Ok(HttpResponse::NotFound().finish());
    };

    // Next, extract action
    let Some(action) = extract_action(&req, &resource) else {
        return Ok(HttpResponse::NotFound().finish());
    };

    // Fail, if action requires params, but params aren't provided, or vice versa.
    if body_params.is_some() != action.requires_params() {
        return Ok(HttpResponse::BadRequest().finish());
    }

    let Some(user) = extract_user(&state.api, user, user_share, &action, &resource).await? else {
        return Err(SecutilsError::access_forbidden());
    };

    let user_id = user.id;
    let params = body_params.map(|body| UtilsActionParams::json(body.into_inner()));
    let action_result = match resource {
        UtilsResource::CertificatesTemplates | UtilsResource::CertificatesPrivateKeys => {
            certificates_handle_action(user, &state.api, action, resource, params).await
        }
        UtilsResource::WebhooksResponders => {
            webhooks_handle_action(user, &state.api, action, resource, params).await
        }
        UtilsResource::WebScrapingPage => {
            web_scraping_handle_action(user, &state.api, action, resource, params).await
        }
        UtilsResource::WebSecurityContentSecurityPolicies => {
            web_security_handle_action(user, &state.api, action, resource, params).await
        }
    };

    match action_result {
        Ok(action_result) => Ok(if let Some(result) = action_result.into_inner() {
            HttpResponse::Ok().json(result)
        } else {
            match action {
                UtilsAction::List | UtilsAction::Get { .. } => HttpResponse::NotFound().finish(),
                UtilsAction::Create
                | UtilsAction::Update { .. }
                | UtilsAction::Delete { .. }
                | UtilsAction::Share { .. }
                | UtilsAction::Unshare { .. }
                | UtilsAction::Execute { .. } => HttpResponse::NoContent().finish(),
            }
        }),
        Err(err) => {
            error!(
                "User ({}) failed to perform utility action: {err:?}",
                *user_id
            );
            Err(err.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_action, extract_resource, extract_user, utils_action};
    use crate::{
        tests::{mock_api, mock_app_state, mock_user, mock_user_with_id},
        users::{SharedResource, UserShare, UserShareId},
        utils::{
            UtilsAction, UtilsResource, UtilsResourceOperation,
            certificates::{PrivateKeyAlgorithm, tests::PrivateKeysCreateParams},
        },
    };
    use actix_web::{body::MessageBody, http::Method, test::TestRequest, web};
    use insta::assert_debug_snapshot;
    use serde_json::json;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_extract_resource() {
        for (area, resource) in [
            (None, None),
            (Some("certificates"), None),
            (None, Some("private_keys")),
            (Some("certificates"), Some("unknown")),
            (Some("webhooks"), None),
            (Some("web_scraping"), None),
        ] {
            let request = TestRequest::with_uri("https://secutils.dev/api/utils");
            let request = if let Some(area) = area {
                request.param("area", area)
            } else {
                request
            };
            let request = if let Some(resource) = resource {
                request.param("resource", resource)
            } else {
                request
            };

            assert!(extract_resource(&request.to_http_request()).is_none());
        }

        assert_eq!(
            extract_resource(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .param("area", "certificates")
                    .param("resource", "private_keys")
                    .to_http_request(),
            ),
            Some(UtilsResource::CertificatesPrivateKeys)
        );
        assert_eq!(
            extract_resource(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .param("area", "certificates")
                    .param("resource", "templates")
                    .to_http_request(),
            ),
            Some(UtilsResource::CertificatesTemplates)
        );
        assert_eq!(
            extract_resource(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .param("area", "webhooks")
                    .param("resource", "responders")
                    .to_http_request(),
            ),
            Some(UtilsResource::WebhooksResponders)
        );
        assert_eq!(
            extract_resource(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .param("area", "web_scraping")
                    .param("resource", "page")
                    .to_http_request(),
            ),
            Some(UtilsResource::WebScrapingPage)
        );
    }

    #[test]
    fn ignores_invalid_actions() {
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");
        for resource in [
            UtilsResource::CertificatesPrivateKeys,
            UtilsResource::CertificatesTemplates,
            UtilsResource::WebhooksResponders,
            UtilsResource::WebScrapingPage,
            UtilsResource::WebSecurityContentSecurityPolicies,
        ] {
            assert!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::PUT)
                        .to_http_request(),
                    &resource,
                )
                .is_none()
            );

            assert!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::POST)
                        .param("resource_id", resource_id.to_string())
                        .to_http_request(),
                    &resource,
                )
                .is_none()
            );
        }
    }

    #[test]
    fn can_extract_common_actions() {
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");
        for resource in [
            UtilsResource::CertificatesPrivateKeys,
            UtilsResource::CertificatesTemplates,
            UtilsResource::WebhooksResponders,
            UtilsResource::WebScrapingPage,
            UtilsResource::WebSecurityContentSecurityPolicies,
        ] {
            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::GET)
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::List)
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::POST)
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Create)
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::GET)
                        .param("resource_id", resource_id.to_string())
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Get { resource_id })
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::PUT)
                        .param("resource_id", resource_id.to_string())
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Update { resource_id })
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::DELETE)
                        .param("resource_id", resource_id.to_string())
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Delete { resource_id })
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::POST)
                        .param("resource_id", resource_id.to_string())
                        .param("resource_operation", "share")
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Share { resource_id })
            );

            assert_eq!(
                extract_action(
                    &TestRequest::with_uri("https://secutils.dev/api/utils")
                        .method(Method::POST)
                        .param("resource_id", resource_id.to_string())
                        .param("resource_operation", "unshare")
                        .to_http_request(),
                    &resource,
                ),
                Some(UtilsAction::Unshare { resource_id })
            );
        }
    }

    #[test]
    fn can_extract_certificates_templates_actions() {
        let resource = UtilsResource::CertificatesTemplates;
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "generate")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::CertificatesTemplateGenerate
            })
        );

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", "peer_certificates")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: None,
                operation: UtilsResourceOperation::CertificatesTemplatePeerCertificates
            })
        );
    }

    #[test]
    fn can_extract_certificates_private_keys_action() {
        let resource = UtilsResource::CertificatesPrivateKeys;
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "export")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::CertificatesPrivateKeyExport
            })
        );
    }

    #[test]
    fn can_extract_webhooks_responders_action() {
        let resource = UtilsResource::WebhooksResponders;
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "history")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebhooksRespondersGetHistory
            })
        );

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "clear")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebhooksRespondersClearHistory
            })
        );

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::GET)
                    .param("resource_id", "stats")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: None,
                operation: UtilsResourceOperation::WebhooksRespondersGetStats
            })
        );
    }

    #[test]
    fn can_extract_web_scraping_page_action() {
        let resource = UtilsResource::WebScrapingPage;
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "history")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingPageGetHistory
            })
        );

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "clear")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingPageClearHistory
            })
        );
    }

    #[test]
    fn can_extract_web_security_content_security_policies_actions() {
        let resource = UtilsResource::WebSecurityContentSecurityPolicies;
        let resource_id = uuid!("00000000-0000-0000-0000-000000000000");

        assert_eq!(
            extract_action(
                &TestRequest::with_uri("https://secutils.dev/api/utils")
                    .method(Method::POST)
                    .param("resource_id", resource_id.to_string())
                    .param("resource_operation", "serialize")
                    .to_http_request(),
                &resource,
            ),
            Some(UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize
            })
        );
    }

    #[sqlx::test]
    async fn can_extract_user(pool: PgPool) -> anyhow::Result<()> {
        let resource_id = uuid!("00000000-0000-0000-0000-000000000001");
        let resource = UtilsResource::CertificatesTemplates;
        let action = UtilsAction::Get { resource_id };

        let api = mock_api(pool).await?;

        // Insert user into the database.
        let user = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000001"))?;
        api.db.upsert_user(&user).await?;

        // No user information.
        assert!(
            extract_user(&api, None, None, &action, &resource)
                .await?
                .is_none()
        );

        // Only current user is provided.
        let extracted_user =
            extract_user(&api, Some(user.clone()), None, &action, &resource).await?;
        assert_eq!(extracted_user.unwrap().id, user.id);

        // Both current user and user share that belongs to that user were provided.
        let extracted_user = extract_user(
            &api,
            Some(user.clone()),
            Some(UserShare {
                id: UserShareId::new(),
                user_id: user.id,
                resource: SharedResource::CertificateTemplate {
                    template_id: resource_id,
                },
                created_at: OffsetDateTime::now_utc(),
            }),
            &action,
            &resource,
        )
        .await?;
        assert_eq!(extracted_user.unwrap().id, user.id);

        // Both current user and user share that doesn't belong to that user were provided.
        let another_user = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        api.db.upsert_user(&another_user).await?;
        let extracted_user = extract_user(
            &api,
            Some(user.clone()),
            Some(UserShare {
                id: UserShareId::new(),
                user_id: another_user.id,
                resource: SharedResource::CertificateTemplate {
                    template_id: resource_id,
                },
                created_at: OffsetDateTime::now_utc(),
            }),
            &action,
            &resource,
        )
        .await?;
        assert_eq!(extracted_user.unwrap().id, another_user.id);

        // Anonymous user.
        let extracted_user = extract_user(
            &api,
            None,
            Some(UserShare {
                id: UserShareId::new(),
                user_id: another_user.id,
                resource: SharedResource::CertificateTemplate {
                    template_id: resource_id,
                },
                created_at: OffsetDateTime::now_utc(),
            }),
            &action,
            &resource,
        )
        .await?;
        assert_eq!(extracted_user.unwrap().id, another_user.id);

        // Current user isn't authorized.
        let another_user = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        api.db.upsert_user(&another_user).await?;
        let extracted_user = extract_user(
            &api,
            Some(user.clone()),
            Some(UserShare {
                id: UserShareId::new(),
                user_id: another_user.id,
                resource: SharedResource::CertificateTemplate {
                    template_id: resource_id,
                },
                created_at: OffsetDateTime::now_utc(),
            }),
            &UtilsAction::Create,
            &resource,
        )
        .await?;
        assert!(extracted_user.is_none());

        // Anonymous user is not authorized.
        let extracted_user = extract_user(
            &api,
            None,
            Some(UserShare {
                id: UserShareId::new(),
                user_id: another_user.id,
                resource: SharedResource::CertificateTemplate {
                    template_id: resource_id,
                },
                created_at: OffsetDateTime::now_utc(),
            }),
            &UtilsAction::Create,
            &resource,
        )
        .await?;
        assert!(extracted_user.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn fail_if_resource_is_not_valid(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::GET)
            .param("area", "certificates")
            .param("resource", "unknown")
            .to_http_request();
        assert_debug_snapshot!(
            utils_action(web::Data::new(app_state), Some(user), None, request, None).await,
            @r###"
        Ok(
            HttpResponse {
                error: None,
                res: 
                Response HTTP/1.1 404 Not Found
                  headers:
                  body: Sized(0)
                ,
            },
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fail_if_action_is_not_valid(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::DELETE)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .to_http_request();
        assert_debug_snapshot!(
            utils_action(web::Data::new(app_state), Some(user), None, request, None).await,
            @r###"
        Ok(
            HttpResponse {
                error: None,
                res: 
                Response HTTP/1.1 404 Not Found
                  headers:
                  body: Sized(0)
                ,
            },
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fail_if_action_requires_body_but_not_provided(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::POST)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .to_http_request();
        assert_debug_snapshot!(
            utils_action(web::Data::new(app_state), Some(user), None, request, None).await,
            @r###"
        Ok(
            HttpResponse {
                error: None,
                res: 
                Response HTTP/1.1 400 Bad Request
                  headers:
                  body: Sized(0)
                ,
            },
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fail_if_user_is_not_authenticated(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::GET)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .to_http_request();
        assert_debug_snapshot!(
            utils_action(web::Data::new(app_state), None, None, request, None).await,
            @r###"
        Err(
            "Access Forbidden",
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fail_if_action_parameters_are_invalid(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::POST)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .to_http_request();
        let body = web::Json(json!({}));
        assert_debug_snapshot!(
            utils_action(web::Data::new(app_state), Some(user), None, request, Some(body)).await,
            @r###"
        Err(
            Error {
                context: "Invalid action parameters.",
                source: Error("missing field `keyName`", line: 0, column: 0),
            },
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_return_json_value(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::POST)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .to_http_request();
        let body = web::Json(json!({ "keyName": "pk", "alg": { "keyType": "ed25519" } }));
        let response = utils_action(
            web::Data::new(app_state),
            Some(user),
            None,
            request,
            Some(body),
        )
        .await?;
        assert_eq!(response.status(), 200);
        assert_debug_snapshot!(response.headers(), @r###"
        HeaderMap {
            inner: {
                "content-type": Value {
                    inner: [
                        "application/json",
                    ],
                },
            },
        }
        "###);

        assert!(!response.into_body().try_into_bytes().unwrap().is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_return_no_value(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let certificates = app_state.api.certificates();
        let private_key = certificates
            .create_private_key(
                user.id,
                PrivateKeysCreateParams {
                    key_name: "pk".to_string(),
                    alg: PrivateKeyAlgorithm::Ed25519,
                    passphrase: None,
                },
            )
            .await?;

        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::PUT)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .param("resource_id", private_key.id.to_string())
            .to_http_request();
        let body = web::Json(json!({ "keyName": "pk-new" }));
        assert_debug_snapshot!(
            utils_action(
                web::Data::new(app_state),
                Some(user),
                None,
                request,
                Some(body),
            )
            .await,
            @r###"
        Ok(
            HttpResponse {
                error: None,
                res: 
                Response HTTP/1.1 204 No Content
                  headers:
                  body: Sized(0)
                ,
            },
        )
        "###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_return_not_found(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let non_existent_id = uuid!("00000000-0000-0000-0000-000000000001");
        let request = TestRequest::with_uri("https://secutils.dev/api/utils")
            .method(Method::GET)
            .param("area", "certificates")
            .param("resource", "private_keys")
            .param("resource_id", non_existent_id.to_string())
            .to_http_request();
        assert_debug_snapshot!(
            utils_action(
                web::Data::new(app_state),
                Some(user),
                None,
                request,
                None,
            )
            .await,
            @r###"
        Ok(
            HttpResponse {
                error: None,
                res: 
                Response HTTP/1.1 404 Not Found
                  headers:
                  body: Sized(0)
                ,
            },
        )
        "###
        );

        Ok(())
    }
}
