use crate::{error::Error as SecutilsError, scheduler::ScheduleExt, server::AppState, users::User};
use actix_web::{web, HttpResponse};
use anyhow::anyhow;
use cron::Schedule;
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds, TimestampSeconds};
use std::{str::FromStr, time::Duration};
use time::OffsetDateTime;

#[derive(Deserialize)]
pub struct SchedulerParseScheduleParams {
    pub schedule: String,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerParseScheduleResult {
    /// The minimum interval between two consequent scheduled tracker checks.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub min_interval: Duration,
    /// The next 5 occurrences of the provided schedule.
    #[serde_as(as = "Vec<TimestampSeconds<i64>>")]
    pub next_occurrences: Vec<OffsetDateTime>,
}

/// Parses the provided schedule and returns the minimum interval between occurrences and the next
/// 5 occurrences.
pub async fn scheduler_parse_schedule(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<SchedulerParseScheduleParams>,
) -> Result<HttpResponse, SecutilsError> {
    // First, try parse schedule as cron expression.
    let schedule = match Schedule::from_str(&body_params.schedule) {
        Ok(schedule) => schedule,
        Err(err) => {
            log::error!(user:serde = user.log_context(); "Failed to parse schedule: {err}");
            return Ok(HttpResponse::BadRequest().body(err.to_string()));
        }
    };

    let features = user.subscription.get_features(&state.config);
    let min_interval = schedule.min_interval()?;
    if min_interval < features.config.web_scraping.min_schedule_interval {
        log::error!(
            user:serde = user.log_context();
            "The minimum interval between occurrences should be greater than {}, but got {}",
            humantime::format_duration(features.config.web_scraping.min_schedule_interval),
            humantime::format_duration(min_interval)
        );
        return Ok(HttpResponse::BadRequest().body(format!(
            "The minimum interval between occurrences should be greater than {}, but got {}",
            humantime::format_duration(features.config.web_scraping.min_schedule_interval),
            humantime::format_duration(min_interval)
        )));
    }

    Ok(HttpResponse::Ok().json(SchedulerParseScheduleResult {
        min_interval,
        next_occurrences: schedule
            .upcoming(chrono::Utc)
            .take(5)
            .map(|ts| {
                OffsetDateTime::from_unix_timestamp(ts.timestamp())
                    .map_err(|_| anyhow!("Failed to calculate next occurrence."))
            })
            .collect::<Result<_, _>>()?,
    }))
}

#[cfg(test)]
mod tests {
    use crate::{
        server::handlers::{
            scheduler_parse_schedule,
            scheduler_parse_schedule::{
                SchedulerParseScheduleParams, SchedulerParseScheduleResult,
            },
        },
        tests::{mock_app_state, mock_app_state_with_config, mock_config, mock_user},
    };
    use actix_web::{body::MessageBody, web};
    use bytes::Bytes;
    use sqlx::PgPool;
    use std::time::Duration;
    use time::OffsetDateTime;

    #[sqlx::test]
    async fn fails_if_schedule_is_invalid(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let response = scheduler_parse_schedule(
            web::Data::new(app_state),
            user,
            web::Json(SchedulerParseScheduleParams {
                schedule: "0 * * * *".to_string(),
            }),
        )
        .await?;
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.into_body().try_into_bytes().unwrap(),
            Bytes::from_static(b"Invalid expression: Invalid cron expression.")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn fails_if_schedule_min_interval_is_less_than_allowed_by_subscription(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config
            .subscriptions
            .ultimate
            .web_scraping
            .min_schedule_interval = Duration::from_secs(3600);

        let app_state = mock_app_state_with_config(pool, config).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let response = scheduler_parse_schedule(
            web::Data::new(app_state),
            user,
            web::Json(SchedulerParseScheduleParams {
                schedule: "0 * * * * *".to_string(),
            }),
        )
        .await?;
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.into_body().try_into_bytes().unwrap(),
            Bytes::from_static(
                b"The minimum interval between occurrences should be greater than 1h, but got 1m"
            )
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_parse_schedule(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let response = scheduler_parse_schedule(
            web::Data::new(app_state),
            user,
            web::Json(SchedulerParseScheduleParams {
                schedule: "0 1 2 3 4 Sat 2050/2".to_string(),
            }),
        )
        .await?;
        assert_eq!(response.status(), 200);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(
            serde_json::from_slice::<SchedulerParseScheduleResult>(&body)?,
            SchedulerParseScheduleResult {
                min_interval: Duration::from_secs(189_302_400),
                next_occurrences: vec![
                    OffsetDateTime::from_unix_timestamp(2848183260)?,
                    OffsetDateTime::from_unix_timestamp(3037485660)?,
                    OffsetDateTime::from_unix_timestamp(3731796060)?,
                    OffsetDateTime::from_unix_timestamp(3921098460)?,
                    OffsetDateTime::from_unix_timestamp(4110400860)?
                ]
            }
        );

        Ok(())
    }
}
