use crate::{
    error::Error as SecutilsError,
    notifications::{NotificationContent, NotificationContentTemplate, NotificationDestination},
    retrack::tags::{
        RETRACK_NOTIFICATIONS_TAG, RETRACK_RESOURCE_ID_TAG, RETRACK_RESOURCE_TAG, RETRACK_USER_TAG,
        get_tag_value,
    },
    security::Operator,
    server::AppState,
    users::UserId,
    utils::UtilsResource,
};
use actix_web::{HttpResponse, web};
use retrack_types::trackers::{WebhookActionPayload, WebhookActionPayloadResult};
use similar::TextDiff;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::{error, info};
use uuid::Uuid;

fn normalize_for_diff(value: &serde_json::Value) -> String {
    if value.is_object() || value.is_array() {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    } else if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

fn compute_unified_diff(old: &str, new: &str, context_radius: usize) -> String {
    TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(context_radius)
        .missing_newline_hint(false)
        .to_string()
}

pub async fn webhooks_retrack(
    state: web::Data<AppState>,
    operator: Operator,
    body_params: web::Json<WebhookActionPayload>,
) -> Result<HttpResponse, SecutilsError> {
    // 1. Retrieve Retrack tracker for the revision.
    let Some(retrack_tracker) = state
        .api
        .retrack()
        .get_tracker(body_params.tracker_id)
        .await?
    else {
        error!(
            operator = operator.id(),
            retrack.id = %body_params.tracker_id,
            retrack.name = body_params.tracker_name,
            "Failed to find tracker to handle Retrack webhook request."
        );
        return Ok(HttpResponse::NotFound().finish());
    };

    // 2. Retrieve user id.
    let Some(Ok(user_id)) =
        get_tag_value(&retrack_tracker.tags, RETRACK_USER_TAG).map(|tag| UserId::from_str(&tag))
    else {
        error!(
            operator = operator.id(),
            retrack.id = %body_params.tracker_id,
            retrack.name = body_params.tracker_name,
            retrack.tags = ?retrack_tracker.tags,
            "Failed to find or parse user ID."
        );
        return Ok(HttpResponse::NotFound().finish());
    };

    // 3. Retrieve user by user ID to make sure it exists.
    let Some(user) = state.api.users().get(user_id).await? else {
        error!(
            operator = operator.id(),
            user.id = %user_id,
            retrack.id = %retrack_tracker.id,
            retrack.name = retrack_tracker.name,
            retrack.tags = ?retrack_tracker.tags,
            "Failed to find user to handle Retrack webhook request."
        );
        return Ok(HttpResponse::NotFound().finish());
    };

    // 4. Retrieve resource that uses this tracker.
    let Some(Ok(util_resource)) = get_tag_value(&retrack_tracker.tags, RETRACK_RESOURCE_TAG)
        .map(|tag| UtilsResource::from_str(&tag))
    else {
        error!(
            user.id = %user.id,
            retrack.id = %retrack_tracker.id,
            retrack.name = retrack_tracker.name,
            retrack.tags = ?retrack_tracker.tags,
            "Failed to find or parse resource."
        );
        return Ok(HttpResponse::NotFound().finish());
    };

    // 5. Retrieve resource ID of the resource that uses this tracker.
    let (resource, resource_group) = util_resource.into();
    let Some(Ok(resource_id)) = get_tag_value(&retrack_tracker.tags, RETRACK_RESOURCE_ID_TAG)
        .map(|tag| Uuid::from_str(&tag))
    else {
        error!(
            user.id = %user.id,
            util.resource = resource,
            util.resource_group = resource_group,
            retrack.id = %retrack_tracker.id,
            retrack.name = retrack_tracker.name,
            retrack.tags = ?retrack_tracker.tags,
            "Failed to find or parse resource ID."
        );
        return Ok(HttpResponse::NotFound().finish());
    };

    let notifications = get_tag_value(&retrack_tracker.tags, RETRACK_NOTIFICATIONS_TAG)
        .and_then(|tag| tag.parse::<bool>().ok())
        .unwrap_or_default();
    info!(
        user.id = %user.id,
        util.resource = resource,
        util.resource_group = resource_group,
        util.resource_id = %resource_id,
        retrack.id = %retrack_tracker.id,
        retrack.name = %retrack_tracker.name,
        retrack.tags = ?retrack_tracker.tags,
        "Webhook is invoked (notification: {notifications}): {:?}.",
        body_params.result
    );

    if !notifications {
        return Ok(HttpResponse::Ok().finish());
    }

    let notification = match util_resource {
        UtilsResource::WebScrapingPage => {
            // 6. Retrieve page tracker by resource ID.
            let Some(tracker) = state
                .api
                .web_scraping(&user)
                .get_page_tracker(resource_id)
                .await?
            else {
                error!(
                    user.id = %user.id,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    util.resource_id = %resource_id,
                    retrack.id = %retrack_tracker.id,
                    retrack.name = retrack_tracker.name,
                    retrack.tags = ?retrack_tracker.tags,
                    "Failed to find page tracker to handle Retrack webhook request."
                );
                return Ok(HttpResponse::NotFound().finish());
            };

            let (content, diff) = match &body_params.result {
                WebhookActionPayloadResult::Success(revision) => {
                    if let Some(new_content) = revision.get("newContent") {
                        let new_normalized = normalize_for_diff(new_content);
                        let diff = revision
                            .get("previousContent")
                            .filter(|p| !p.is_null())
                            .map(|prev| {
                                compute_unified_diff(
                                    &normalize_for_diff(prev),
                                    &new_normalized,
                                    state.api.config.utils.diff_context_radius,
                                )
                            });
                        (Ok(new_normalized), diff)
                    } else {
                        (Ok(normalize_for_diff(revision)), None)
                    }
                }
                WebhookActionPayloadResult::Failure(err) => (Err(err.to_string()), None),
            };

            NotificationContent::Template(NotificationContentTemplate::PageTrackerChanges {
                tracker_id: tracker.id,
                tracker_name: tracker.name,
                content,
                diff,
            })
        }
        _ => {
            error!(
                user.id = %user.id,
                util.resource = resource,
                util.resource_group = resource_group,
                util.resource_id = %resource_id,
                retrack.id = %retrack_tracker.id,
                retrack.name = retrack_tracker.name,
                retrack.tags = ?retrack_tracker.tags,
                "Webhook is not supported for this resource."
            );
            return Ok(HttpResponse::BadRequest().body(format!(
                "Webhook is not supported for the resource: {util_resource}"
            )));
        }
    };

    let notification_schedule_result = state
        .api
        .notifications()
        .schedule_notification(
            NotificationDestination::User(user.id),
            notification,
            OffsetDateTime::now_utc(),
        )
        .await;
    if let Err(err) = notification_schedule_result {
        error!(
            user.id = %user.id,
            util.resource = resource,
            util.resource_group = resource_group,
            util.resource_id = %resource_id,
            retrack.id = %retrack_tracker.id,
            retrack.name = retrack_tracker.name,
            retrack.tags = ?retrack_tracker.tags,
            "Failed to schedule a notification for the tracker: {err:?}."
        );
    }

    Ok(HttpResponse::Ok().finish())
}

#[cfg(test)]
mod tests {
    use super::{compute_unified_diff, webhooks_retrack};
    use crate::{
        notifications::{
            Notification, NotificationContent, NotificationContentTemplate, NotificationDestination,
        },
        retrack::{
            RetrackTracker,
            tags::{
                RETRACK_NOTIFICATIONS_TAG, RETRACK_RESOURCE_ID_TAG, RETRACK_RESOURCE_NAME_TAG,
                RETRACK_RESOURCE_TAG, RETRACK_USER_TAG, prepare_tags,
            },
            tests::mock_retrack_tracker,
        },
        security::Operator,
        tests::{mock_app_state_with_config, mock_config, mock_user},
        utils::{UtilsResource, web_scraping::tests::MockPageTrackerBuilder},
    };
    use actix_web::{body::MessageBody, web};
    use bytes::Bytes;
    use futures::StreamExt;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use retrack_types::trackers::{WebhookActionPayload, WebhookActionPayloadResult};
    use serde_json::json;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[sqlx::test]
    async fn fails_for_unknown_retrack_trackers(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let app_state = web::Data::new(app_state);

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/api/trackers/00000000-0000-0000-0000-000000000001");
            then.status(404).header("Content-Type", "application/json");
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "tracker".to_string(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_not_specified_users(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let app_state = web::Data::new(app_state);

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_unknown_users(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[format!(
            "{RETRACK_USER_TAG}:{}",
            uuid!("00000000-0000-0000-0000-000000000001")
        )]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_unknown_resources(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:some-resource"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_not_specified_resource_id(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn skips_notification_if_notification_is_disabled(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            format!(
                "{RETRACK_RESOURCE_ID_TAG}:{}",
                uuid!("00000000-0000-0000-0000-000000000001")
            ),
            format!("{RETRACK_RESOURCE_NAME_TAG}:{}", retrack_tracker.name),
            format!("{RETRACK_NOTIFICATIONS_TAG}:false"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_not_supported_resources(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!(
                "{RETRACK_RESOURCE_TAG}:{}",
                UtilsResource::WebhooksResponders
            ),
            format!(
                "{RETRACK_RESOURCE_ID_TAG}:{}",
                uuid!("00000000-0000-0000-0000-000000000001")
            ),
            format!("{RETRACK_RESOURCE_NAME_TAG}:{}", retrack_tracker.name),
            format!("{RETRACK_NOTIFICATIONS_TAG}:true"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 400 Bad Request
              headers:
              body: Sized(63)
            ,
        }
        "###);
        assert_eq!(
            response.into_body().try_into_bytes().unwrap(),
            Bytes::from_static(b"Webhook is not supported for the resource: webhooks__responders")
        );

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn fails_for_unknown_trackers(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            format!(
                "{RETRACK_RESOURCE_ID_TAG}:{}",
                uuid!("00000000-0000-0000-0000-000000000001")
            ),
            format!("{RETRACK_RESOURCE_NAME_TAG}:{}", retrack_tracker.name),
            format!("{RETRACK_NOTIFICATIONS_TAG}:true"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({})),
            }),
        )
        .await?;
        retrack_get_api_mock.assert();

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 404 Not Found
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10);
        assert!(notifications.collect::<Vec<_>>().await.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn can_schedule_success_notification(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            retrack_tracker.name.clone(),
            RetrackTracker::from_value(retrack_tracker.clone()),
        )?
        .build();
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
            format!("{RETRACK_RESOURCE_NAME_TAG}:{}", tracker.name),
            format!("{RETRACK_NOTIFICATIONS_TAG}:true"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        app_state
            .api
            .db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Success(json!({
                    "newContent": { "one": 1 },
                    "previousContent": { "one": 0 }
                })),
            }),
        )
        .await?;
        retrack_get_api_mock.assert_calls(2);

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let mut notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(notifications.len(), 1);

        let notification = app_state
            .api
            .db
            .get_notification(notifications.remove(0)?)
            .await?
            .unwrap();

        let expected_content = serde_json::to_string_pretty(&json!({ "one": 1 })).unwrap();
        let expected_previous = serde_json::to_string_pretty(&json!({ "one": 0 })).unwrap();
        let expected_diff = compute_unified_diff(&expected_previous, &expected_content, 3);
        assert_eq!(
            notification,
            Notification {
                id: notification.id,
                destination: NotificationDestination::User(mock_user.id),
                content: NotificationContent::Template(
                    NotificationContentTemplate::PageTrackerChanges {
                        tracker_id: tracker.id,
                        tracker_name: tracker.name.clone(),
                        content: Ok(expected_content),
                        diff: Some(expected_diff),
                    }
                ),
                scheduled_at: notification.scheduled_at
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_schedule_failure_notification(pool: PgPool) -> anyhow::Result<()> {
        let retrack_server = MockServer::start();
        let mut config = mock_config()?;
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let app_state = mock_app_state_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        app_state.api.db.insert_user(&mock_user).await?;

        let app_state = web::Data::new(app_state);

        let mut retrack_tracker = mock_retrack_tracker()?;
        let tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            retrack_tracker.name.clone(),
            RetrackTracker::from_value(retrack_tracker.clone()),
        )?
        .build();
        retrack_tracker.tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
            format!("{RETRACK_RESOURCE_NAME_TAG}:{}", tracker.name),
            format!("{RETRACK_NOTIFICATIONS_TAG}:true"),
        ]);
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        app_state
            .api
            .db
            .web_scraping(mock_user.id)
            .insert_page_tracker(&tracker)
            .await?;

        let response = webhooks_retrack(
            app_state.clone(),
            Operator::new("operator"),
            web::Json(WebhookActionPayload {
                tracker_id: retrack_tracker.id,
                tracker_name: retrack_tracker.name.clone(),
                result: WebhookActionPayloadResult::Failure("some error".to_string()),
            }),
        )
        .await?;
        retrack_get_api_mock.assert_calls(2);

        assert_debug_snapshot!(response, @r###"
        HttpResponse {
            error: None,
            res: 
            Response HTTP/1.1 200 OK
              headers:
              body: Sized(0)
            ,
        }
        "###);

        let mut notifications = app_state
            .api
            .db
            .get_notification_ids(OffsetDateTime::now_utc(), 10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(notifications.len(), 1);

        let notification = app_state
            .api
            .db
            .get_notification(notifications.remove(0)?)
            .await?
            .unwrap();
        assert_eq!(
            notification,
            Notification {
                id: notification.id,
                destination: NotificationDestination::User(mock_user.id),
                content: NotificationContent::Template(
                    NotificationContentTemplate::PageTrackerChanges {
                        tracker_id: tracker.id,
                        tracker_name: tracker.name.clone(),
                        content: Err("some error".to_string()),
                        diff: None,
                    }
                ),
                scheduled_at: notification.scheduled_at
            }
        );

        Ok(())
    }
}
