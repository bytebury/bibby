//! Helpers for the database tests (`#[sqlx::test]`). Each of those tests gets
//! its own fresh database with every migration applied, so they run in
//! parallel and never see each other's rows.

use crate::domain::user::{AuthProvider, CreateUser, Role, User};
use crate::infra::db::DbPool;

/// A verified email account with `role`, signed in from `ip`.
pub async fn insert_user(db: &DbPool, email: &str, role: Role, ip: &str) -> User {
    User::create(
        db,
        &CreateUser {
            country_id: None,
            region_id: None,
            city_id: None,
            email: email.to_string(),
            first_name: "Test".to_string(),
            last_name: Some("User".to_string()),
            full_name: "Test User".to_string(),
            image_url: String::new(),
            role,
            verified: true,
            auth_provider: AuthProvider::Email,
            locked: false,
            last_known_ip: ip.to_string(),
        },
    )
    .await
    .expect("insert test user")
}
