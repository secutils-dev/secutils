use crate::{
    error::Error as SecutilsError, scheduler::CronExt, server::AppState, users::User,
    utils::web_scraping::expand_schedule_preset,
};
use actix_web::{HttpResponse, post, web};
use anyhow::anyhow;
use croner::{Cron, Direction};
use serde_derive::{Deserialize, Serialize};
use serde_with::{DurationMilliSeconds, TimestampSeconds, serde_as};
use std::time::Duration;
use time::OffsetDateTime;
use tracing::error;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"schedule": "0 0 * * * *"}))]
pub struct SchedulerParseScheduleParams {
    /// A cron expression to parse (6 or 7 fields with seconds).
    pub schedule: String,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerParseScheduleResult {
    /// The minimum interval between two consequent scheduled tracker checks (milliseconds).
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    #[schema(value_type = u64)]
    pub min_interval: Duration,
    /// The next 5 occurrences of the provided schedule (unix timestamps).
    #[serde_as(as = "Vec<TimestampSeconds<i64>>")]
    #[schema(value_type = Vec<i64>)]
    pub next_occurrences: Vec<OffsetDateTime>,
}

/// Parses a cron schedule and returns the minimum interval and next occurrences.
#[utoipa::path(
    tags = ["scheduler"],
    request_body = SchedulerParseScheduleParams,
    responses(
        (status = 200, description = "Parsed schedule information.", body = SchedulerParseScheduleResult),
        (status = BAD_REQUEST, description = "Invalid schedule or interval too small.")
    )
)]
#[post("/api/scheduler/parse_schedule")]
pub async fn scheduler_parse_schedule(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<SchedulerParseScheduleParams>,
) -> Result<HttpResponse, SecutilsError> {
    scheduler_parse_schedule_inner(&state, &user, &body_params)
}

pub(crate) fn scheduler_parse_schedule_inner(
    state: &web::Data<AppState>,
    user: &User,
    body_params: &SchedulerParseScheduleParams,
) -> Result<HttpResponse, SecutilsError> {
    // Expand preset aliases to anchored cron expressions so the preview matches actual scheduling.
    let effective_schedule = expand_schedule_preset(&body_params.schedule);
    let schedule = match Cron::parse_pattern(&effective_schedule) {
        Ok(schedule) => schedule,
        Err(err) => {
            error!(user.id = %user.id, "Failed to parse schedule: {err}");
            return Ok(HttpResponse::BadRequest().body(err.to_string()));
        }
    };

    let features = user.subscription.get_features(&state.config);
    let min_interval = schedule.min_interval()?;
    if min_interval < features.config.web_scraping.min_schedule_interval {
        error!(
            user.id = %user.id,
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
            .iter_from(chrono::Utc::now(), Direction::Forward)
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
    use super::{
        SchedulerParseScheduleParams, SchedulerParseScheduleResult, scheduler_parse_schedule_inner,
    };
    use crate::tests::{
        mock_app_state, mock_app_state_with_config, mock_config, mock_user, schema_example,
    };
    use actix_web::{body::MessageBody, web};
    use bytes::Bytes;
    use sqlx::PgPool;
    use std::time::Duration;
    use time::OffsetDateTime;

    #[test]
    fn scheduler_parse_schedule_params_example_is_valid() {
        let example: SchedulerParseScheduleParams =
            serde_json::from_value(schema_example::<SchedulerParseScheduleParams>()).unwrap();
        assert!(!example.schedule.is_empty());
    }

    #[sqlx::test]
    async fn fails_if_schedule_is_invalid(pool: PgPool) -> anyhow::Result<()> {
        let app_state = mock_app_state(pool).await?;

        let user = mock_user()?;
        app_state.api.db.upsert_user(&user).await?;

        let response = scheduler_parse_schedule_inner(
            &web::Data::new(app_state),
            &user,
            &SchedulerParseScheduleParams {
                schedule: "0 * * * *".to_string(),
            },
        )?;
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.into_body().try_into_bytes().unwrap(),
            Bytes::from_static(
                b"Invalid pattern: Pattern must have 6 or 7 fields when seconds are required and years are optional."
            )
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

        let response = scheduler_parse_schedule_inner(
            &web::Data::new(app_state),
            &user,
            &SchedulerParseScheduleParams {
                schedule: "0 * * * * *".to_string(),
            },
        )?;
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

        let response = scheduler_parse_schedule_inner(
            &web::Data::new(app_state),
            &user,
            &SchedulerParseScheduleParams {
                schedule: "0 1 2 3 4 Sat".to_string(),
            },
        )?;
        assert_eq!(response.status(), 200);

        let body = response.into_body().try_into_bytes().unwrap();
        assert_eq!(
            serde_json::from_slice::<SchedulerParseScheduleResult>(&body)?,
            SchedulerParseScheduleResult {
                min_interval: Duration::from_secs(157_852_800),
                next_occurrences: vec![
                    OffsetDateTime::from_unix_timestamp(1806717660)?,
                    OffsetDateTime::from_unix_timestamp(1964570460)?,
                    OffsetDateTime::from_unix_timestamp(2153872860)?,
                    OffsetDateTime::from_unix_timestamp(2501028060)?,
                    OffsetDateTime::from_unix_timestamp(2690330460)?
                ]
            }
        );

        Ok(())
    }
}
