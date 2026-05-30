use crate::domain::user::{Role, User};
use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::prelude::*;
use crate::use_cases::payments::checkout_use_case::Plan;
use axum::http::HeaderMap;
use shima::webhook::event::ShimaEvent;
use std::str::FromStr;

pub struct HandleStripeEventUseCase {
    stripe: Stripe,
    db: SharedDatabase,
}

impl HandleStripeEventUseCase {
    pub fn new(stripe: &Stripe, db: SharedDatabase) -> Self {
        Self {
            stripe: stripe.clone(),
            db,
        }
    }

    pub async fn execute(&self, headers: &HeaderMap, body: &str) -> Result<()> {
        match self.stripe.listener.process(headers, body)? {
            ShimaEvent::CheckoutSessionCompleted(event) => {
                self.on_checkout_completed(&event).await?;
            }
            ShimaEvent::CustomerSubscriptionDeleted(event)
            | ShimaEvent::InvoicePaymentFailed(event) => {
                self.downgrade_to_free(&event).await?;
            }
            ShimaEvent::Other(_) => {}
        };

        Ok(())
    }

    async fn on_checkout_completed(&self, event: &serde_json::Value) -> Result<()> {
        let customer_id = event
            .get("customer")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::BadRequest("checkout.session.completed missing customer".into())
            })?;
        let plan_str = event
            .get("client_reference_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::BadRequest(
                    "checkout.session.completed missing client_reference_id".into(),
                )
            })?;
        let plan = Plan::from_str(plan_str)?;
        let user = User::find_by_stripe_customer_id(self.db.as_ref(), customer_id).await?;
        User::set_role(self.db.as_ref(), user.id, &plan.role()).await?;
        Ok(())
    }

    async fn downgrade_to_free(&self, event: &serde_json::Value) -> Result<()> {
        let customer_id = event
            .get("customer")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("stripe event missing customer".into()))?;
        let user = User::find_by_stripe_customer_id(self.db.as_ref(), customer_id).await?;
        User::set_role(self.db.as_ref(), user.id, &Role::Free).await?;
        Ok(())
    }
}
