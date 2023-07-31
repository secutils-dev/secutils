use anyhow::Context;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

#[derive(Clone)]
pub struct PrimaryDb {
    pub(crate) pool: Pool<Sqlite>,
}

/// Common methods for primary DB, extensions are implemented separately in every module.
impl PrimaryDb {
    /// Opens primary DB "connection".
    pub async fn open<I: FnOnce() -> anyhow::Result<String>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(&initializer()?).await?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .with_context(|| "Failed to migrate database")?;

        Ok(PrimaryDb { pool })
    }
}
