use crate::infra::payments::Stripe;
use crate::prelude::*;
use shima::{CustomerId, delete_customer};

pub struct DeleteCustomerUseCase {
    stripe: Stripe,
}

impl DeleteCustomerUseCase {
    pub fn new(stripe: &Stripe) -> Self {
        Self {
            stripe: stripe.clone(),
        }
    }

    /// Deletes a Stripe customer, which also cancels every subscription tied
    /// to that customer.
    pub async fn execute(&self, customer_id: &CustomerId) -> Result<()> {
        // Ids decoded from the database skip shima's validation, so a blank
        // legacy row would otherwise hit `DELETE /v1/customers/`.
        if customer_id.as_str().trim().is_empty() {
            return Ok(());
        }
        delete_customer!(&self.stripe.client, customer_id)?;
        Ok(())
    }
}
