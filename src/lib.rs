use crate::domain::value_objects::admin_emails::AdminEmails;
use crate::infra::assets::AssetManifest;
use crate::infra::auth::jwt::Jwt;
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
#[cfg(test)]
mod test_support;
mod use_cases;

pub use error::AppError;

pub struct AppState {
    pub db: SharedDatabase,
    pub app_name: String,
    pub assets: AssetManifest,
    pub seo: SeoConfig,
    pub web_app: WebAppConfig,
    pub user_use_cases: UserUseCases,
    /// `None` when `STRIPE_SECRET_KEY` isn't set: billing is then switched off
    /// and its routes answer 404.
    pub payments_use_cases: Option<PaymentsUseCases>,
}

impl AppState {
    pub fn new(db: SharedDatabase) -> Self {
        Jwt::ensure_configured();
        let app_name = env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string());
        Self {
            user_use_cases: UserUseCases::new(db.clone(), Arc::new(AdminEmails::from_env())),
            payments_use_cases: Stripe::from_env()
                .map(|stripe| PaymentsUseCases::new(&stripe, db.clone())),
            seo: SeoConfig::from_env(&app_name),
            web_app: WebAppConfig::from_env(&app_name),
            app_name,
            assets: AssetManifest::load("public"),
            db,
        }
    }
}

impl AppState {
    pub fn payments(&self) -> Result<&PaymentsUseCases> {
        self.payments_use_cases
            .as_ref()
            .ok_or_else(|| AppError::NotFound("Payments aren't set up here.".to_string()))
    }
}

pub type SharedState = Arc<AppState>;
