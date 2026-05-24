use crate::{error::Error, notifications::SECUTILS_LOGO_BYTES, server::app_state::AppState};
use actix_web::{HttpResponse, get, post, web};
use base64ct::{Base64, Encoding};
use serde::Deserialize;
use serde_json::json;
use std::sync::LazyLock;
use utoipa::{IntoParams, ToSchema};

/// The secutils-logo as a base64-encoded PNG, computed once at startup. We embed it as a
/// `data:` URI in the confirmation page so the page is fully self-contained: it renders
/// correctly without relying on the webui assets being reachable, which matters because the
/// unsubscribe link is followed by a user who may have stale/invalid auth state, browser
/// extensions blocking the SPA bundle, or even just a flaky connection.
static SECUTILS_LOGO_DATA_URI_BASE64: LazyLock<String> =
    LazyLock::new(|| Base64::encode_string(SECUTILS_LOGO_BYTES));

/// Body of the RFC 8058 one-click unsubscribe POST. Mail clients send the form-encoded
/// `List-Unsubscribe=One-Click` value, but our endpoint additionally accepts a JSON body that
/// carries the token so a UI confirmation page can call it from the browser without scraping
/// query params.
#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"token": "BPshNd1bvP72jY-bxmGLZGwj9D5GrPcA"}))]
pub struct NotificationsUnsubscribeParams {
    pub token: String,
}

/// Query-string variant accepted by the GET surface (mail clients that follow the
/// `List-Unsubscribe: <https://...>` URL on click).
#[derive(Deserialize, Debug, Clone, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsUnsubscribeQuery {
    pub token: String,
}

/// One-click unsubscribe endpoint. Authentication is intentionally absent: the bearer of the
/// token is the only proof of intent we have, mirroring how every transactional-email vendor
/// implements RFC 8058. We always return 204, regardless of whether the token exists or has
/// already been used, to avoid leaking an enumeration oracle.
#[utoipa::path(
    tags = ["notifications"],
    request_body = NotificationsUnsubscribeParams,
    responses(
        (status = 204, description = "Unsubscribe request accepted (token may or may not exist, the response is identical either way).")
    ),
    security(())
)]
#[post("/api/notifications/unsubscribe")]
pub async fn notifications_unsubscribe(
    state: web::Data<AppState>,
    body: web::Json<NotificationsUnsubscribeParams>,
) -> Result<HttpResponse, Error> {
    handle_post(&state, body.into_inner()).await
}

/// Identical to the POST endpoint, but accessible via GET so that legacy mail clients which
/// only follow the `List-Unsubscribe: <https://...>` URL on click also work. The token is
/// supplied as a query parameter.
///
/// Unlike the POST surface (which returns 204 per RFC 8058 §3.1), the GET surface is what a
/// human's browser actually navigates to when they click the visible unsubscribe link in the
/// email body. Returning an empty 204 there leaves the user staring at a blank tab with no
/// confirmation that anything happened, so we render a small server-side branded page that
/// reuses the email-styles partial for visual continuity. The page is intentionally
/// self-contained (logo embedded as a base64 `data:` URI, no JS, no SPA dependency) so it
/// works even when the user is signed out, on a flaky connection, or behind extensions that
/// would block the webui bundle.
#[utoipa::path(
    tags = ["notifications"],
    params(NotificationsUnsubscribeQuery),
    responses(
        (status = 200, description = "Unsubscribe request accepted, an HTML confirmation page is returned.", content_type = "text/html")
    ),
    security(())
)]
#[get("/api/notifications/unsubscribe")]
pub async fn notifications_unsubscribe_get(
    state: web::Data<AppState>,
    query: web::Query<NotificationsUnsubscribeQuery>,
) -> Result<HttpResponse, Error> {
    handle_get(&state, query.into_inner()).await
}

/// Plain-async helpers wrapping the body of each handler. The actix `#[get]`/`#[post]`
/// macros wrap their target function in a `HttpServiceFactory` struct that is no longer
/// callable, so unit tests can't drive the macro-decorated functions directly. Splitting
/// the logic out keeps the handlers thin while letting tests reach the rendering and
/// side-effect surface.
async fn handle_post(
    state: &AppState,
    params: NotificationsUnsubscribeParams,
) -> Result<HttpResponse, Error> {
    state
        .api
        .notification_destinations_system()
        .unsubscribe_by_token(&params.token)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn handle_get(
    state: &AppState,
    query: NotificationsUnsubscribeQuery,
) -> Result<HttpResponse, Error> {
    state
        .api
        .notification_destinations_system()
        .unsubscribe_by_token(&query.token)
        .await?;

    let body = state
        .api
        .templates
        .render(
            "notifications_unsubscribe_confirmation",
            &json!({
                "home_link": state.api.config.public_url.as_str(),
                "logo_data_uri": SECUTILS_LOGO_DATA_URI_BASE64.as_str(),
            }),
        )
        .map_err(anyhow::Error::from)?;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tests::{mock_app_state, mock_user, schema_example},
        users::{
            NotificationChannelKind,
            notification_destinations_tests::{PendingDestinationUpsert, verification_expiry},
        },
    };
    use actix_web::{
        body::MessageBody,
        http::{StatusCode, header},
    };
    use sqlx::PgPool;
    use time::OffsetDateTime;

    #[test]
    fn unsubscribe_params_example_is_valid() {
        let example: NotificationsUnsubscribeParams =
            serde_json::from_value(schema_example::<NotificationsUnsubscribeParams>()).unwrap();
        assert!(!example.token.is_empty());
    }

    /// Convenience: read a synchronous-`MessageBody` `HttpResponse` body into a `String`.
    /// Production handlers always synthesise the body with `web::Bytes`/`String`, so this is
    /// safe.
    fn body_as_string(response: HttpResponse) -> String {
        let bytes = response.into_body().try_into_bytes().unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    /// Seeds a fully-verified notification destination directly via the DB and returns the
    /// unsubscribe token. Mirrors the helper used in the notifications-API tests.
    async fn seed_verified_destination(
        db: &crate::database::Database,
        user_id: crate::users::UserId,
        token: &str,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::from_unix_timestamp(1700000000)?;
        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc-test-hash",
            verification_expires_at: verification_expiry(now),
            verification_sent_at: now,
            unsubscribe_token: token,
            now,
        })
        .await?;
        db.mark_notification_destination_verified(user_id, NotificationChannelKind::Email, now)
            .await?;
        Ok(())
    }

    #[sqlx::test]
    async fn get_returns_branded_html_confirmation_page(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let app_state = mock_app_state(pool).await?;
        app_state.api.db.upsert_user(&user).await?;
        seed_verified_destination(&app_state.api.db, user.id, "tok-render").await?;

        let response = handle_get(
            &app_state,
            NotificationsUnsubscribeQuery {
                token: "tok-render".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );

        let body = body_as_string(response);
        // Branded confirmation page: heading, copy, brand-coloured CTA back into the app,
        // and the inline logo data URI. The CTA points at the configured public URL so the
        // user lands on the SPA when they choose to navigate away. The heading text uses a
        // raw apostrophe in the template which Handlebars passes through verbatim (it only
        // HTML-escapes *interpolated* values).
        assert!(
            body.contains("<h1>You've been unsubscribed</h1>"),
            "page should render the success heading"
        );
        assert!(
            body.contains(
                "You will no longer receive Secutils.dev product notifications at this address."
            ),
            "page should explain what just happened"
        );
        assert!(
            body.contains(
                "Account activation, password recovery, and other security messages still go to your login email."
            ),
            "page should reassure that security-critical mail is unaffected"
        );
        assert!(
            body.contains(r#"href="https://secutils.dev/">Return to Secutils.dev</a>"#),
            "page should link back to the configured public URL"
        );
        assert!(
            body.contains("data:image/png;base64,"),
            "page should embed the logo as a base64 data URI"
        );
        assert!(
            body.contains(r#"<meta name="robots" content="noindex, nofollow">"#),
            "an unauthenticated success page should not be indexed by search engines"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn get_marks_destination_unsubscribed_in_db(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let app_state = mock_app_state(pool).await?;
        app_state.api.db.upsert_user(&user).await?;
        seed_verified_destination(&app_state.api.db, user.id, "tok-side-effect").await?;

        let _ = handle_get(
            &app_state,
            NotificationsUnsubscribeQuery {
                token: "tok-side-effect".to_string(),
            },
        )
        .await?;

        let after = app_state
            .api
            .db
            .get_notification_destination_by_unsubscribe_token("tok-side-effect")
            .await?
            .expect("destination should still exist after unsubscribe");
        assert!(
            after.unsubscribed_at.is_some(),
            "GET handler must persist the unsubscribed_at timestamp on the row"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn get_returns_html_even_for_unknown_token(pool: PgPool) -> anyhow::Result<()> {
        // Do NOT seed any destination. We want to confirm the handler returns the same
        // 200/HTML response for an unknown token as for a real one — preserving the
        // anti-enumeration property of the underlying `unsubscribe_by_token` call.
        let app_state = mock_app_state(pool).await?;

        let response = handle_get(
            &app_state,
            NotificationsUnsubscribeQuery {
                token: "definitely-not-a-real-token".to_string(),
            },
        )
        .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        let body = body_as_string(response);
        assert!(body.contains("You've been unsubscribed"));

        Ok(())
    }

    #[sqlx::test]
    async fn post_one_click_endpoint_still_returns_204(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let app_state = mock_app_state(pool).await?;
        app_state.api.db.upsert_user(&user).await?;
        seed_verified_destination(&app_state.api.db, user.id, "tok-rfc-8058").await?;

        // RFC 8058 §3.1 mandates a 200 or 2xx with no body for the One-Click POST. We
        // emit 204 No Content; the test pins this behaviour so a future refactor can't
        // accidentally change the POST surface to render the HTML page.
        let response = handle_post(
            &app_state,
            NotificationsUnsubscribeParams {
                token: "tok-rfc-8058".to_string(),
            },
        )
        .await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        // The response carries no `Content-Type` header — `HttpResponse::NoContent`
        // intentionally strips body framing.
        let body = body_as_string(response);
        assert!(body.is_empty(), "POST One-Click response must be empty");

        Ok(())
    }
}
