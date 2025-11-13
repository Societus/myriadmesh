use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;
use tracing::info;

/// Persistent storage manager
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("myriadnode.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        info!("Opening database: {}", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;

        // Run migrations
        Self::migrate(&pool).await?;

        Ok(Self { pool })
    }

    async fn migrate(pool: &SqlitePool) -> Result<()> {
        info!("Running database migrations...");

        // Create messages table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                destination TEXT NOT NULL,
                payload BLOB NOT NULL,
                priority INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create adapters table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS adapters (
                name TEXT PRIMARY KEY,
                adapter_type TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                last_seen INTEGER
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create metrics table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                adapter_name TEXT NOT NULL,
                destination TEXT NOT NULL,
                latency_ms REAL,
                bandwidth_bps REAL,
                reliability REAL,
                timestamp INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        info!("Database migrations complete");
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        // Pool will be closed when dropped
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
