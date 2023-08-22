use cron::Schedule;

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Clone, Debug)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `ResourcesTrackersSchedule` job.
    pub resources_trackers_schedule: Schedule,
    /// The schedule to use for the `ResourcesTrackersFetch` job.
    pub resources_trackers_fetch: Schedule,
    /// The schedule to use for the `NotificationsSend` job.
    pub notifications_send: Schedule,
}
