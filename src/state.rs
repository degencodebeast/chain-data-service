use crate::cache::AppCache;
use crate::config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub config: Config,
    pub db_pool: SqlitePool,
    pub cache: Arc<Mutex<AppCache>>,
}
