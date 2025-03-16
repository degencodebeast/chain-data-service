// Implement SQLite connection pool
// Create initialization function
// Set up migration function to create tables

use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};
use std::env;
use crate::db::INIT_SCHEMA;

pub async fn establish_connection() -> Result<Pool<Sqlite>, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data.db".to_string());

    // Create database if it doesn't exist
    if !Sqlite::database_exists(&database_url).await.unwrap_or(false) {
        Sqlite::create_database(&database_url).await?;
    }

    // Create connection pool
    let pool = SqlitePool::connect(&database_url).await?;
    
    // Enable WAL mode for better concurrency
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    
    // Initialize schema
    sqlx::query(INIT_SCHEMA).execute(&pool).await?;

    Ok(pool)
}