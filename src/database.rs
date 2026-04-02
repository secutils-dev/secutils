use anyhow::Context;
use sqlx::{PgPool, Pool, Postgres};

#[derive(Clone)]
pub struct Database {
    pub(crate) pool: Pool<Postgres>,
}

/// Common methods for the primary database, extensions are implemented separately in every module.
impl Database {
    /// Opens database "connection".
    pub async fn create(pool: PgPool) -> anyhow::Result<Self> {
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .with_context(|| "Failed to migrate database")?;

        Ok(Database { pool })
    }

    /// Returns `true` if the database is reachable.
    pub async fn is_alive(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}

impl AsRef<Database> for Database {
    fn as_ref(&self) -> &Self {
        self
    }
}
