use anyhow::Context;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use std::path::Path;

#[derive(Clone)]
pub struct Database {
    pub(crate) pool: Pool<Sqlite>,
}

/// Common methods for the primary database, extensions are implemented separately in every module.
impl Database {
    /// Opens database "connection".
    pub async fn open<I: FnOnce() -> anyhow::Result<String>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(&initializer()?).await?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .with_context(|| "Failed to migrate database")?;

        Ok(Database { pool })
    }

    pub async fn open_path<P: AsRef<Path>>(data_path: P) -> anyhow::Result<Self> {
        Self::open(|| {
            data_path
                .as_ref()
                .to_str()
                .ok_or_else(|| {
                    anyhow::anyhow!("Cannot stringify database folder {:?}", data_path.as_ref())
                })
                .map(|db_dir| format!("sqlite:{db_dir}/data.db?mode=rwc"))
        })
        .await
    }
}

impl AsRef<Database> for Database {
    fn as_ref(&self) -> &Self {
        self
    }
}
