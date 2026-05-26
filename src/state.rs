use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub db: SqlitePool,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}
