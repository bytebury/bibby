use crate::domain::user::User;
use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::prelude::*;
use shima::customer::CreateCustomer;
use shima::{CustomerId, create_customer};

pub struct CreateCustomerUseCase {
    stripe: Stripe,
    db: SharedDatabase,
}

impl CreateCustomerUseCase {
    pub fn new(stripe: &Stripe, db: SharedDatabase) -> Self {
        Self {
            stripe: stripe.clone(),
            db,
        }
    }

    pub async fn execute(&self, user: &User) -> Result<CustomerId> {
        if let Some(existing) = &user.stripe_customer_id {
            return CustomerId::try_from(existing.clone())
                .map_err(|e| AppError::Internal(format!("Invalid stored Stripe customer id: {e}")));
        }
        let customer = CreateCustomer::new(&user.full_name, &user.email);
        let customer_id = create_customer!(&self.stripe.client, customer)?.id;
        User::set_stripe_customer_id(self.db.as_ref(), user.id, &customer_id).await?;
        Ok(customer_id)
    }
}
