use sqlx::{migrate::MigrateDatabase, sqlite::{SqliteConnectOptions, SqliteJournalMode}, Pool, Sqlite, SqlitePool};
use std::{env, str::FromStr};

use crate::db::INIT_SCHEMA;

pub async fn establish_connection() -> Result<Pool<Sqlite>, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        // Use absolute path to prevent directory confusion
        format!("sqlite:{}/data.db", std::env::current_dir().unwrap().display())
    });

    // Log the actual database path being used
    tracing::info!("Using database at: {}", database_url);
    
    // Create database if it doesn't exist
    if !Sqlite::database_exists(&database_url).await.unwrap_or(false) {
        Sqlite::create_database(&database_url).await?;
    }

    // Create connection pool with connection options for better reliability
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::from_str(&database_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
    ).await?;
    
    // Initialize schema
    sqlx::query(INIT_SCHEMA).execute(&pool).await?;

    Ok(pool)
}

