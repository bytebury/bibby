use crate::infra::db::SharedDatabase;
use crate::use_cases::user::UserUseCases;
use std::sync::Arc;
use crate::prelude::*;

mod domain;
mod error;
pub mod infra;
mod prelude;
mod use_cases;

pub use error::AppError;

pub struct AppState {
    pub db: SharedDatabase,
    pub app_name: String,
    pub app_version: String,
    pub user_use_cases: UserUseCases,
}

impl AppState {
    pub fn new(db: SharedDatabase) -> Self {
        Self {
            user_use_cases: UserUseCases::new(db.clone()),
            app_name: env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string()),
            app_version: uuid::Uuid::new_v4().simple().to_string(),
            db,
        }
    }
}

pub type SharedState = Arc<AppState>;
