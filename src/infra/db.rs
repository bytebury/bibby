use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

pub type DbPool = PgPool;
pub type SharedDatabase = Arc<DbPool>;
pub type PrimaryKey = i32;

pub struct Database {}

impl Database {
    pub async fn init() -> DbPool {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        // Per replica, so several replicas need a Postgres that allows
        // replicas × this many connections.
        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(25);

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .idle_timeout(Duration::from_secs(5 * 60))
            .max_lifetime(Duration::from_secs(30 * 60))
            .connect(&database_url)
            .await
            .expect("Failed to connect to database.");

        tracing::info!("PostgreSQL connected successfully.");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        tracing::info!("Database migrations have finished successfully.");

        pool
    }
}
