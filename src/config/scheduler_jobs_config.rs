use cron::Schedule;

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Clone, Debug)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `ResourcesTrackersDispatch` job.
    pub resources_trackers_dispatch_schedule: Schedule,
}
