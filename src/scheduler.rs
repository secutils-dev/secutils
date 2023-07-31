mod scheduler_store;
use tokio_cron_scheduler::{JobScheduler, SimpleJobCode, SimpleNotificationCode};

use crate::datastore::Datastore;
use scheduler_store::SchedulerStore;

pub struct Scheduler {
    #[allow(dead_code)]
    inner_scheduler: JobScheduler,
}

impl Scheduler {
    pub async fn start(datastore: Datastore) -> anyhow::Result<Self> {
        let scheduler = JobScheduler::new_with_storage_and_code(
            Box::new(SchedulerStore::new(datastore.primary_db.clone())),
            Box::new(SchedulerStore::new(datastore.primary_db.clone())),
            Box::<SimpleJobCode>::default(),
            Box::<SimpleNotificationCode>::default(),
        )
        .await?;

        scheduler.start().await?;

        Ok(Self {
            inner_scheduler: scheduler,
        })
    }
}
