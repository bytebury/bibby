use crate::infra::db::SharedDatabase;
use crate::use_cases::user::register_user_use_case::RegisterUserUseCase;
use crate::use_cases::user::search_use_case::SearchUseCase;
use crate::use_cases::user::send_sign_in_code_use_case::SendSignInCodeUseCase;
use crate::use_cases::user::sign_up_with_email_use_case::SignUpWithEmailUseCase;
use crate::use_cases::user::verify_sign_in_code_use_case::VerifySignInCodeUseCase;

pub mod register_user_use_case;
pub mod search_use_case;
pub mod send_sign_in_code_use_case;
pub mod sign_up_with_email_use_case;
pub mod verify_sign_in_code_use_case;

pub struct UserUseCases {
    pub register: RegisterUserUseCase,
    pub search: SearchUseCase,
    pub send_sign_in_code: SendSignInCodeUseCase,
    pub sign_up_with_email: SignUpWithEmailUseCase,
    pub verify_sign_in_code: VerifySignInCodeUseCase,
}

impl UserUseCases {
    pub fn new(db: SharedDatabase) -> Self {
        Self {
            register: RegisterUserUseCase::new(db.clone()),
            search: SearchUseCase::new(db.clone()),
            send_sign_in_code: SendSignInCodeUseCase::new(db.clone()),
            sign_up_with_email: SignUpWithEmailUseCase::new(db.clone()),
            verify_sign_in_code: VerifySignInCodeUseCase::new(db),
        }
    }
}
