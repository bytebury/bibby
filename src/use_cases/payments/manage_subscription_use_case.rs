use crate::infra::payments::Stripe;
use crate::prelude::*;
use crate::use_cases::payments::checkout_use_case::app_origin;
use shima::billing::{CreateCustomerPortalSession, CustomerPortalSession};
use shima::{CustomerId, ReturnUrl, manage_subscriptions};

pub struct ManageSubscriptionUseCase {
    stripe: Stripe,
}

impl ManageSubscriptionUseCase {
    pub fn new(stripe: &Stripe) -> Self {
        Self {
            stripe: stripe.clone(),
        }
    }

    pub async fn execute(&self, customer_id: CustomerId) -> Result<CustomerPortalSession> {
        let website_url = app_origin()?;
        let return_url = ReturnUrl::from(website_url);
        let session = CreateCustomerPortalSession::new(customer_id, return_url);
        manage_subscriptions!(&self.stripe.client, session).map_err(Into::into)
    }
}
