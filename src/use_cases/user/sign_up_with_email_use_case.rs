use crate::domain::avatar;
use crate::domain::user::{AuthProvider, CreateUser, Role, SignUp, User};
use crate::infra::db::SharedDatabase;
use crate::prelude::*;
use crate::use_cases::user::register_user_use_case::RegisterUserUseCase;
use crate::use_cases::user::send_sign_in_code_use_case::SendSignInCodeUseCase;

/// Creates an account for someone who signs up with an email address instead
/// of Google, then emails them a code to verify it.
///
/// The account starts unverified and can't hold a session until the code is
/// entered on `/sign-in/code/verify`, which is also how it signs in from then
/// on. Like the sign-in form, every outcome looks the same to the caller — an
/// address that already has an account just gets a code — so the form can't
/// be used to discover who has signed up.
pub struct SignUpWithEmailUseCase {
    db: SharedDatabase,
    register: RegisterUserUseCase,
    send_code: SendSignInCodeUseCase,
}

impl SignUpWithEmailUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self {
            register: RegisterUserUseCase::new(db.clone()),
            send_code: SendSignInCodeUseCase::new(db.clone()),
            db,
        }
    }

    pub async fn execute(&self, sign_up: &SignUp, ip: &str) -> Result<()> {
        match User::find_by_email(self.db.as_ref(), sign_up.email.as_str()).await {
            Ok(user) => {
                // Nobody has proven they own this address yet, so the latest
                // sign-up decides what the account is called.
                if !user.verified && user.signs_in_with_email() {
                    User::update_unverified_name(
                        self.db.as_ref(),
                        user.id,
                        &sign_up.first_name,
                        &sign_up.last_name,
                    )
                    .await?;
                }
            }
            Err(AppError::NotFound(_)) => self.create(sign_up, ip).await?,
            Err(error) => return Err(error),
        }

        self.send_code.execute(&sign_up.email).await
    }

    async fn create(&self, sign_up: &SignUp, ip: &str) -> Result<()> {
        let mut request = CreateUser {
            country_id: None,
            region_id: None,
            email: sign_up.email.to_string(),
            first_name: sign_up.first_name.clone(),
            last_name: Some(sign_up.last_name.clone()),
            full_name: format!("{} {}", sign_up.first_name, sign_up.last_name),
            image_url: avatar::random(),
            role: Role::Free,
            verified: false,
            auth_provider: AuthProvider::Email,
            locked: false,
            last_known_ip: ip.to_string(),
        };
        self.register.resolve_location(&mut request).await?;

        match User::create(self.db.as_ref(), &request).await {
            // Two sign-ups for the same address raced; either account will do.
            Ok(_) | Err(AppError::Conflict(_)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
