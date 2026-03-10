use retrack_types::scheduler::SchedulerJobConfig;
use time::OffsetDateTime;

/// Expands preset schedule aliases (`@hourly`, `@daily`, `@weekly`, `@monthly`) into
/// creation-time-anchored 6-field cron expressions so that the first run is always ~1 full
/// interval away. Non-preset values pass through unchanged.
pub fn expand_schedule_preset(schedule: &str) -> String {
    let now = OffsetDateTime::now_utc();
    match schedule {
        "@hourly" => format!("0 {} * * * *", now.minute()),
        "@daily" => format!("0 {} {} * * *", now.minute(), now.hour()),
        "@weekly" => format!(
            "0 {} {} * * {}",
            now.minute(),
            now.hour(),
            now.weekday().number_days_from_sunday()
        ),
        "@monthly" => format!(
            "0 {} {} {} * *",
            now.minute(),
            now.hour(),
            now.day().min(28)
        ),
        other => other.to_string(),
    }
}

/// Returns a new `SchedulerJobConfig` with the schedule preset expanded to an anchored cron
/// expression (if applicable), preserving all other fields.
pub fn expand_job_config(job: SchedulerJobConfig) -> SchedulerJobConfig {
    SchedulerJobConfig {
        schedule: expand_schedule_preset(&job.schedule),
        ..job
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_non_preset_schedules() {
        assert_eq!(
            expand_schedule_preset("0 30 9 * * Mon,Wed"),
            "0 30 9 * * Mon,Wed"
        );
        assert_eq!(expand_schedule_preset("0 0 * * * *"), "0 0 * * * *");
        assert_eq!(expand_schedule_preset("custom-string"), "custom-string");
    }

    #[test]
    fn expands_hourly_preset() {
        let result = expand_schedule_preset("@hourly");
        let parts: Vec<&str> = result.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "0");
        assert!(parts[1].parse::<u8>().unwrap() < 60);
        assert_eq!(parts[2], "*");
        assert_eq!(parts[3], "*");
        assert_eq!(parts[4], "*");
        assert_eq!(parts[5], "*");
    }

    #[test]
    fn expands_daily_preset() {
        let result = expand_schedule_preset("@daily");
        let parts: Vec<&str> = result.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "0");
        assert!(parts[1].parse::<u8>().unwrap() < 60);
        assert!(parts[2].parse::<u8>().unwrap() < 24);
        assert_eq!(parts[3], "*");
        assert_eq!(parts[4], "*");
        assert_eq!(parts[5], "*");
    }

    #[test]
    fn expands_weekly_preset() {
        let result = expand_schedule_preset("@weekly");
        let parts: Vec<&str> = result.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "0");
        assert!(parts[1].parse::<u8>().unwrap() < 60);
        assert!(parts[2].parse::<u8>().unwrap() < 24);
        assert_eq!(parts[3], "*");
        assert_eq!(parts[4], "*");
        assert!(parts[5].parse::<u8>().unwrap() <= 6);
    }

    #[test]
    fn expands_monthly_preset() {
        let result = expand_schedule_preset("@monthly");
        let parts: Vec<&str> = result.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "0");
        assert!(parts[1].parse::<u8>().unwrap() < 60);
        assert!(parts[2].parse::<u8>().unwrap() < 24);
        let day = parts[3].parse::<u8>().unwrap();
        assert!((1..=28).contains(&day));
        assert_eq!(parts[4], "*");
        assert_eq!(parts[5], "*");
    }

    #[test]
    fn expand_job_config_preserves_retry_strategy() {
        use retrack_types::scheduler::SchedulerJobRetryStrategy;
        use std::time::Duration;

        let job = SchedulerJobConfig {
            schedule: "@daily".to_string(),
            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(120),
                max_attempts: 5,
            }),
        };
        let expanded = expand_job_config(job);
        assert_ne!(expanded.schedule, "@daily");
        assert!(expanded.schedule.split(' ').count() == 6);
        assert!(expanded.retry_strategy.is_some());
    }
}
