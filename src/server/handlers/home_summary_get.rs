use crate::{error::Error as SecutilsError, server::AppState, users::User};
use actix_web::{HttpResponse, web};

pub async fn home_summary_get(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, SecutilsError> {
    Ok(HttpResponse::Ok().json(state.api.utils().get_home_summary(user.id).await?))
}

#[cfg(test)]
mod tests {
    use crate::{
        server::handlers::home_summary_get,
        tests::{MockResponderBuilder, mock_app_state, mock_user},
        utils::HomeSummary,
    };
    use actix_web::{body::MessageBody, web};
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn returns_empty_summary_for_authenticated_user(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let response = home_summary_get(web::Data::new(app_state), user).await?;
        assert_eq!(response.status(), 200);

        let body = response.into_body().try_into_bytes().unwrap();
        let summary: HomeSummary = serde_json::from_slice(&body)?;

        assert_eq!(summary.counts.webhooks, 0);
        assert_eq!(summary.counts.certificates, 0);
        assert_eq!(summary.counts.csp, 0);
        assert_eq!(summary.counts.web_scraping, 0);
        assert!(summary.recent_items.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn returns_summary_with_data(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        app_state
            .api
            .db
            .webhooks()
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "my-responder",
                    "/test",
                )?
                .build(),
            )
            .await?;

        let response = home_summary_get(web::Data::new(app_state), user).await?;
        assert_eq!(response.status(), 200);

        let body = response.into_body().try_into_bytes().unwrap();
        let summary: HomeSummary = serde_json::from_slice(&body)?;

        assert_eq!(summary.counts.webhooks, 1);
        assert_eq!(summary.recent_items.len(), 1);
        assert_eq!(summary.recent_items[0].name, "my-responder");
        assert_eq!(summary.recent_items[0].util_handle, "webhooks__responders");

        Ok(())
    }
}
