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
    pub async fn execute(&self, customer_id: &str) -> Result<()> {
        // A blank id would hit `DELETE /v1/customers/`, which Stripe rejects.
        if customer_id.trim().is_empty() {
            return Ok(());
        }
        let customer_id = CustomerId::try_from(customer_id.to_string())
            .map_err(|e| AppError::Internal(format!("Invalid stored Stripe customer id: {e}")))?;
        delete_customer!(&self.stripe.client, customer_id)?;
        Ok(())
    }
}
