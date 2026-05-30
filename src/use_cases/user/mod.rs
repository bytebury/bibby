use crate::infra::db::SharedDatabase;
use crate::use_cases::user::register_user_use_case::RegisterUserUseCase;
use crate::use_cases::user::search_use_case::SearchUseCase;

pub mod register_user_use_case;
pub mod search_use_case;

pub struct UserUseCases {
    pub register: RegisterUserUseCase,
    pub search: SearchUseCase,
}

impl UserUseCases {
    pub fn new(db: SharedDatabase) -> Self {
        Self {
            register: RegisterUserUseCase::new(db.clone()),
            search: SearchUseCase::new(db),
        }
    }
}
