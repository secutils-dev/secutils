use cron::Schedule;
use std::time::Duration;

pub trait ScheduleExt {
    /// Returns the minimum interval between occurrences.
    fn min_interval(&self) -> anyhow::Result<Duration>;
}

impl ScheduleExt for Schedule {
    /// Returns the minimum interval between occurrences. To calculate it, we take the first 100
    /// upcoming occurrences and calculate the interval between each of them. Then we take the
    /// smallest interval.
    fn min_interval(&self) -> anyhow::Result<Duration> {
        let mut minimum_interval = Duration::MAX;
        let next_occurrences = self.upcoming(chrono::Utc).take(100).collect::<Vec<_>>();
        for (index, occurrence) in next_occurrences.iter().enumerate().skip(1) {
            let interval = (*occurrence - next_occurrences[index - 1]).to_std()?;
            if interval < minimum_interval {
                minimum_interval = interval;
            }
        }

        Ok(minimum_interval)
    }
}

#[cfg(test)]
mod tests {
    use super::ScheduleExt;
    use cron::Schedule;
    use std::{str::FromStr, time::Duration};

    #[test]
    fn can_calculate_min_interval() -> anyhow::Result<()> {
        let schedule = Schedule::from_str("0 * * * * * *")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(60));

        let schedule = Schedule::from_str("0 0 * * * * *")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(3600));
        let schedule = Schedule::from_str("@hourly")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(3600));

        let schedule = Schedule::from_str("0 0 0 * * * *")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(24 * 3600));
        let schedule = Schedule::from_str("@daily")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(24 * 3600));

        let schedule = Schedule::from_str("0 0 0 * * 1 *")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(7 * 24 * 3600));
        let schedule = Schedule::from_str("@weekly")?;
        assert_eq!(schedule.min_interval()?, Duration::from_secs(7 * 24 * 3600));

        Ok(())
    }
}
