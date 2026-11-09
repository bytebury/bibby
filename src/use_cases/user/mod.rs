use crate::infra::db::SharedDatabase;
use crate::use_cases::user::link_accounts_use_case::LinkAccountsUseCase;
use crate::use_cases::user::process_plan_expirations_use_case::ProcessPlanExpirationsUseCase;
use crate::use_cases::user::prune_sign_ins_use_case::PruneSignInsUseCase;
use crate::use_cases::user::register_user_use_case::RegisterUserUseCase;
use crate::use_cases::user::search_use_case::SearchUseCase;
use crate::use_cases::user::send_sign_in_code_use_case::SendSignInCodeUseCase;
use crate::use_cases::user::sign_up_with_email_use_case::SignUpWithEmailUseCase;
use crate::use_cases::user::unlink_account_use_case::UnlinkAccountUseCase;
use crate::use_cases::user::verify_sign_in_code_use_case::VerifySignInCodeUseCase;

pub mod link_accounts_use_case;
pub mod process_plan_expirations_use_case;
pub mod prune_sign_ins_use_case;
pub mod register_user_use_case;
pub mod search_use_case;
pub mod send_sign_in_code_use_case;
pub mod sign_up_with_email_use_case;
pub mod unlink_account_use_case;
pub mod verify_sign_in_code_use_case;

pub struct UserUseCases {
    pub register: RegisterUserUseCase,
    pub search: SearchUseCase,
    pub send_sign_in_code: SendSignInCodeUseCase,
    pub sign_up_with_email: SignUpWithEmailUseCase,
    pub verify_sign_in_code: VerifySignInCodeUseCase,
    pub link_accounts: LinkAccountsUseCase,
    pub unlink_account: UnlinkAccountUseCase,
    pub prune_sign_ins: PruneSignInsUseCase,
    pub process_plan_expirations: ProcessPlanExpirationsUseCase,
}

impl UserUseCases {
    pub fn new(db: SharedDatabase) -> Self {
        Self {
            register: RegisterUserUseCase::new(db.clone()),
            search: SearchUseCase::new(db.clone()),
            send_sign_in_code: SendSignInCodeUseCase::new(db.clone()),
            sign_up_with_email: SignUpWithEmailUseCase::new(db.clone()),
            verify_sign_in_code: VerifySignInCodeUseCase::new(db.clone()),
            link_accounts: LinkAccountsUseCase::new(db.clone()),
            unlink_account: UnlinkAccountUseCase::new(db.clone()),
            prune_sign_ins: PruneSignInsUseCase::new(db.clone()),
            process_plan_expirations: ProcessPlanExpirationsUseCase::new(db),
        }
    }
}
