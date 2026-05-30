use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::infra::seo::{SeoConfig, WebAppConfig};
use crate::prelude::*;
use crate::use_cases::payments::PaymentsUseCases;
use crate::use_cases::user::UserUseCases;
use std::sync::Arc;

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
    pub seo: SeoConfig,
    pub web_app: WebAppConfig,
    pub user_use_cases: UserUseCases,
    pub payments_use_cases: PaymentsUseCases,
}

impl AppState {
    pub fn new(db: SharedDatabase) -> Self {
        let stripe = Stripe::default();
        let app_name = env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string());
        Self {
            user_use_cases: UserUseCases::new(db.clone()),
            payments_use_cases: PaymentsUseCases::new(&stripe, db.clone()),
            seo: SeoConfig::from_env(&app_name),
            web_app: WebAppConfig::from_env(&app_name),
            app_name,
            app_version: uuid::Uuid::new_v4().simple().to_string(),
            db,
        }
    }
}

pub type SharedState = Arc<AppState>;
