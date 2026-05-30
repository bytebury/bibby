pub mod checkout_use_case;
mod create_customer_use_case;
mod handle_stripe_event;
mod manage_subscription_use_case;

use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::use_cases::payments::checkout_use_case::CheckoutUseCase;
use crate::use_cases::payments::create_customer_use_case::CreateCustomerUseCase;
use crate::use_cases::payments::handle_stripe_event::HandleStripeEventUseCase;
use crate::use_cases::payments::manage_subscription_use_case::ManageSubscriptionUseCase;

pub struct PaymentsUseCases {
    pub checkout: CheckoutUseCase,
    pub create_customer: CreateCustomerUseCase,
    pub manage_subscription: ManageSubscriptionUseCase,
    pub handle_stripe_event: HandleStripeEventUseCase,
}

impl PaymentsUseCases {
    pub fn new(stripe: &Stripe, db: SharedDatabase) -> Self {
        Self {
            checkout: CheckoutUseCase::new(stripe),
            create_customer: CreateCustomerUseCase::new(stripe, db.clone()),
            manage_subscription: ManageSubscriptionUseCase::new(stripe),
            handle_stripe_event: HandleStripeEventUseCase::new(stripe, db),
        }
    }
}
