use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

const DATABASE_FILE_NAME: &str = "channarr.db";

pub async fn connect(data_dir: impl AsRef<Path>) -> Result<SqlitePool> {
    let data_dir = data_dir.as_ref();
    let database_path = data_dir.join(DATABASE_FILE_NAME);

    tokio::fs::create_dir_all(data_dir)
        .await
        .with_context(|| format!("failed to create data directory {}", data_dir.display()))?;

    let options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open SQLite database {}", database_path.display()))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;

    tracing::info!(database = %database_path.display(), "database ready");

    Ok(pool)
}
