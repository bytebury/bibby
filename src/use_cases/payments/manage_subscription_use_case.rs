use crate::infra::payments::Stripe;
use crate::prelude::*;
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
        let website_url = env::var("WEBSITE_URL").expect("WEBSITE_URL must be set");
        let return_url = ReturnUrl::from(website_url);
        let session = CreateCustomerPortalSession::new(customer_id, return_url);
        manage_subscriptions!(&self.stripe.client, session).map_err(Into::into)
    }
}
