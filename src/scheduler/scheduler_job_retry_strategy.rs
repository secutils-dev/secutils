use serde::{Deserialize, Serialize};
use serde_with::{DurationMilliSeconds, serde_as};
use std::time::Duration;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum SchedulerJobRetryStrategy {
    /// The job will be retried with a constant interval (1s -> 1s -> 1s).
    #[serde(rename_all = "camelCase")]
    Constant {
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        interval: Duration,
        max_attempts: u32,
    },
    /// The job will be retried with an exponential interval (1s -> 2s -> 4s -> 8s).
    #[serde(rename_all = "camelCase")]
    Exponential {
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        initial_interval: Duration,
        multiplier: u32,
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        max_interval: Duration,
        max_attempts: u32,
    },
    /// The job will be retried with a linear interval (1s -> 2s -> 3s).
    #[serde(rename_all = "camelCase")]
    Linear {
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        initial_interval: Duration,
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        increment: Duration,
        #[serde_as(as = "DurationMilliSeconds<u64>")]
        max_interval: Duration,
        max_attempts: u32,
    },
}

impl SchedulerJobRetryStrategy {
    /// Calculates the interval for the next retry attempt.
    pub fn interval(&self, attempt: u32) -> Duration {
        match self {
            Self::Constant { interval, .. } => *interval,
            Self::Exponential {
                initial_interval,
                multiplier,
                max_interval,
                ..
            } => multiplier
                .checked_pow(attempt)
                .and_then(|multiplier| initial_interval.checked_mul(multiplier))
                .map(|interval| interval.min(*max_interval))
                .unwrap_or_else(|| *max_interval),
            Self::Linear {
                initial_interval,
                increment,
                max_interval,
                ..
            } => increment
                .checked_mul(attempt)
                .and_then(|increment| initial_interval.checked_add(increment))
                .map(|interval| interval.min(*max_interval))
                .unwrap_or_else(|| *max_interval),
        }
    }

    /// Returns the maximum number of attempts.
    pub fn max_attempts(&self) -> u32 {
        match self {
            Self::Constant { max_attempts, .. } => *max_attempts,
            Self::Exponential { max_attempts, .. } => *max_attempts,
            Self::Linear { max_attempts, .. } => *max_attempts,
        }
    }

    /// Returns the minimum retry interval.
    pub fn min_interval(&self) -> &Duration {
        match self {
            Self::Constant { interval, .. } => interval,
            Self::Exponential {
                initial_interval, ..
            }
            | Self::Linear {
                initial_interval, ..
            } => initial_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerJobRetryStrategy;
    use std::time::Duration;

    #[test]
    fn properly_detects_max_number_of_attempts() {
        assert_eq!(
            SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(1),
                max_attempts: 10,
            }
            .max_attempts(),
            10
        );
        assert_eq!(
            SchedulerJobRetryStrategy::Exponential {
                initial_interval: Duration::from_secs(1),
                multiplier: 2,
                max_interval: Duration::from_secs(10),
                max_attempts: 15,
            }
            .max_attempts(),
            15
        );
        assert_eq!(
            SchedulerJobRetryStrategy::Linear {
                initial_interval: Duration::from_secs(1),
                increment: Duration::from_secs(1),
                max_interval: Duration::from_secs(10),
                max_attempts: 20,
            }
            .max_attempts(),
            20
        );
    }

    #[test]
    fn properly_detects_min_interval() {
        assert_eq!(
            SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(1),
                max_attempts: 10,
            }
            .min_interval(),
            &Duration::from_secs(1)
        );
        assert_eq!(
            SchedulerJobRetryStrategy::Exponential {
                initial_interval: Duration::from_secs(2),
                multiplier: 2,
                max_interval: Duration::from_secs(10),
                max_attempts: 15,
            }
            .min_interval(),
            &Duration::from_secs(2)
        );
        assert_eq!(
            SchedulerJobRetryStrategy::Linear {
                initial_interval: Duration::from_secs(3),
                increment: Duration::from_secs(1),
                max_interval: Duration::from_secs(10),
                max_attempts: 20,
            }
            .min_interval(),
            &Duration::from_secs(3)
        );
    }

    #[test]
    fn properly_calculates_constant_interval() {
        let retry_strategy = SchedulerJobRetryStrategy::Constant {
            interval: Duration::from_secs(1),
            max_attempts: 10,
        };
        assert_eq!(retry_strategy.interval(0), Duration::from_secs(1));
        assert_eq!(retry_strategy.interval(1), Duration::from_secs(1));
        assert_eq!(retry_strategy.interval(2), Duration::from_secs(1));
        assert_eq!(retry_strategy.interval(u32::MAX), Duration::from_secs(1));
    }

    #[test]
    fn properly_calculates_linear_interval() {
        let retry_strategy = SchedulerJobRetryStrategy::Linear {
            initial_interval: Duration::from_secs(1),
            increment: Duration::from_secs(1),
            max_interval: Duration::from_secs(5),
            max_attempts: 10,
        };
        assert_eq!(retry_strategy.interval(0), Duration::from_secs(1));
        assert_eq!(retry_strategy.interval(1), Duration::from_secs(2));
        assert_eq!(retry_strategy.interval(2), Duration::from_secs(3));
        assert_eq!(retry_strategy.interval(3), Duration::from_secs(4));
        assert_eq!(retry_strategy.interval(4), Duration::from_secs(5));
        assert_eq!(retry_strategy.interval(5), Duration::from_secs(5));
        assert_eq!(retry_strategy.interval(6), Duration::from_secs(5));
        assert_eq!(retry_strategy.interval(100), Duration::from_secs(5));
        assert_eq!(retry_strategy.interval(u32::MAX), Duration::from_secs(5));
    }

    #[test]
    fn properly_calculates_exponential_interval() {
        let retry_strategy = SchedulerJobRetryStrategy::Exponential {
            initial_interval: Duration::from_secs(1),
            multiplier: 2,
            max_interval: Duration::from_secs(100),
            max_attempts: 10,
        };
        assert_eq!(retry_strategy.interval(0), Duration::from_secs(1));
        assert_eq!(retry_strategy.interval(1), Duration::from_secs(2));
        assert_eq!(retry_strategy.interval(2), Duration::from_secs(4));
        assert_eq!(retry_strategy.interval(3), Duration::from_secs(8));
        assert_eq!(retry_strategy.interval(4), Duration::from_secs(16));
        assert_eq!(retry_strategy.interval(5), Duration::from_secs(32));
        assert_eq!(retry_strategy.interval(6), Duration::from_secs(64));
        assert_eq!(retry_strategy.interval(7), Duration::from_secs(100));
        assert_eq!(retry_strategy.interval(100), Duration::from_secs(100));
        assert_eq!(retry_strategy.interval(u32::MAX), Duration::from_secs(100));
    }
}
