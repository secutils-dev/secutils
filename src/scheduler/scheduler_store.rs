use crate::datastore::PrimaryDb;
use chrono::{DateTime, Utc};
use std::{future::Future, pin::Pin, time::Duration};
use time::OffsetDateTime;
use tokio_cron_scheduler::{
    DataStore, InitStore, JobAndNextTick, JobId, JobNotification, JobSchedulerError, JobStoredData,
    MetaDataStorage, NotificationData, NotificationId, NotificationStore,
};
use uuid::Uuid;

/// Implementation of the SQLite storage for the Tokio scheduler.
pub struct SchedulerStore {
    db: PrimaryDb,
}

impl SchedulerStore {
    pub fn new(db: PrimaryDb) -> Self {
        Self { db }
    }
}

// All tables for Scheduler are created upfront.
impl InitStore for SchedulerStore {
    fn init(&mut self) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn inited(&mut self) -> Pin<Box<dyn Future<Output = Result<bool, JobSchedulerError>> + Send>> {
        Box::pin(std::future::ready(Ok(true)))
    }
}

/// Interface for Jobs data CRUD operations.
impl DataStore<JobStoredData> for SchedulerStore {
    fn get(
        &mut self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Option<JobStoredData>, JobSchedulerError>> + Send>>
    {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_scheduler_job(id).await.map_err(|err| {
                log::error!("Error getting scheduler job: {:?}", err);
                JobSchedulerError::GetJobData
            })
        })
    }

    fn add_or_update(
        &mut self,
        job: JobStoredData,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.upsert_scheduler_job(&job).await.map_err(|err| {
                log::error!("Error updating scheduler job: {:?}", err);
                JobSchedulerError::CantAdd
            })
        })
    }

    fn delete(
        &mut self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.remove_scheduler_job(id).await.map_err(|err| {
                log::error!("Error deleting scheduler job: {:?}", err);
                JobSchedulerError::CantRemove
            })
        })
    }
}

/// Interface for Notifications data CRUD operations.
impl DataStore<NotificationData> for SchedulerStore {
    fn get(
        &mut self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Option<NotificationData>, JobSchedulerError>> + Send>>
    {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_scheduler_notification(id).await.map_err(|err| {
                log::error!("Error getting scheduler notification: {:?}", err);
                JobSchedulerError::GetJobData
            })
        })
    }

    fn add_or_update(
        &mut self,
        notification: NotificationData,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.upsert_scheduler_notification(&notification)
                .await
                .map_err(|err| {
                    log::error!("Error updating scheduler notification: {:?}", err);
                    JobSchedulerError::UpdateJobData
                })
        })
    }

    fn delete(
        &mut self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.remove_scheduler_notification(id).await.map_err(|err| {
                log::error!("Error deleting scheduler notification: {:?}", err);
                JobSchedulerError::CantRemove
            })
        })
    }
}

/// Interface for the jobs metadata manipulations.
impl MetaDataStorage for SchedulerStore {
    fn list_next_ticks(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<JobAndNextTick>, JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_next_scheduler_jobs().await.map_err(|err| {
                log::error!("Error getting next jobs: {:?}", err);
                JobSchedulerError::CantListNextTicks
            })
        })
    }

    fn set_next_and_last_tick(
        &mut self,
        id: Uuid,
        next_tick: Option<DateTime<Utc>>,
        last_tick: Option<DateTime<Utc>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            let next_tick = next_tick
                .map(|tick| OffsetDateTime::from_unix_timestamp(tick.timestamp()))
                .transpose()
                .map_err(|err| {
                    log::error!(
                        "Error updating scheduler job ticks, invalid next tick: {:?}",
                        err
                    );
                    JobSchedulerError::UpdateJobData
                })?;
            let last_tick = last_tick
                .map(|tick| OffsetDateTime::from_unix_timestamp(tick.timestamp()))
                .transpose()
                .map_err(|err| {
                    log::error!(
                        "Error updating scheduler job ticks, invalid last tick: {:?}",
                        err
                    );
                    JobSchedulerError::UpdateJobData
                })?;
            db.set_scheduler_job_ticks(id, next_tick, last_tick)
                .await
                .map_err(|err| {
                    log::error!("Error updating scheduler job ticks: {:?}", err);
                    JobSchedulerError::UpdateJobData
                })
        })
    }

    fn time_till_next_job(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_scheduler_time_until_next_job(OffsetDateTime::now_utc())
                .await
                .map_err(|err| {
                    log::error!("Error getting time until next job: {:?}", err);
                    JobSchedulerError::CouldNotGetTimeUntilNextTick
                })
        })
    }
}

/// Interface for the notifications manipulations.
impl NotificationStore for SchedulerStore {
    fn list_notification_guids_for_job_and_state(
        &mut self,
        job_id: JobId,
        state: JobNotification,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<NotificationId>, JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_scheduler_notification_ids_for_job_and_state(job_id, state)
                .await
                .map_err(|err| {
                    log::error!(
                        "Error getting notification ids for job id and state: {:?}",
                        err
                    );
                    JobSchedulerError::CantListGuids
                })
        })
    }

    fn list_notification_guids_for_job_id(
        &mut self,
        job_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Uuid>, JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.get_scheduler_notification_ids_for_job(job_id)
                .await
                .map_err(|err| {
                    log::error!("Error getting notification ids for job id: {:?}", err);
                    JobSchedulerError::CantListGuids
                })
        })
    }

    fn delete_notification_for_state(
        &mut self,
        notification_id: Uuid,
        state: JobNotification,
    ) -> Pin<Box<dyn Future<Output = Result<bool, JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.remove_scheduler_notification_for_state(notification_id, state)
                .await
                .map_err(|err| {
                    log::error!("Error deleting notification: {:?}", err);
                    JobSchedulerError::CantRemove
                })
        })
    }

    fn delete_for_job(
        &mut self,
        job_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), JobSchedulerError>> + Send>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.remove_scheduler_notification_for_job(job_id)
                .await
                .map_err(|err| {
                    log::error!("Error deleting notification: {:?}", err);
                    JobSchedulerError::CantRemove
                })
        })
    }
}
