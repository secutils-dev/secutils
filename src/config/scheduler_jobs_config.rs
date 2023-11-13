use cron::Schedule;

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Clone, Debug)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `WebPageTrackersSchedule` job.
    pub web_page_trackers_schedule: Schedule,
    /// The schedule to use for the `WebPageTrackersFetch` job.
    pub web_page_trackers_fetch: Schedule,
    /// The schedule to use for the `NotificationsSend` job.
    pub notifications_send: Schedule,
}
