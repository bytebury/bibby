use crate::domain::user::{Role, User};
use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::prelude::*;
use crate::use_cases::payments::checkout_use_case::Plan;
use axum::http::HeaderMap;
use shima::CustomerId;
use shima::checkout::CheckoutSession;
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
            // Delayed payment methods complete the session unpaid and settle later
            // via `async_payment_succeeded`, so only grant access once fulfilled.
            ShimaEvent::CheckoutSessionCompleted(session)
            | ShimaEvent::CheckoutSessionAsyncPaymentSucceeded(session)
                if session.is_fulfilled() =>
            {
                self.on_checkout_fulfilled(&session).await?;
            }
            ShimaEvent::CustomerSubscriptionDeleted(subscription) => {
                self.downgrade_to_free(&subscription.customer).await?;
            }
            ShimaEvent::InvoicePaymentFailed(invoice) => {
                let customer = invoice.customer.ok_or_else(|| {
                    AppError::BadRequest("invoice.payment_failed missing customer".into())
                })?;
                self.downgrade_to_free(&customer).await?;
            }
            _ => {}
        };

        Ok(())
    }

    async fn on_checkout_fulfilled(&self, session: &CheckoutSession) -> Result<()> {
        let customer = session.customer.as_ref().ok_or_else(|| {
            AppError::BadRequest("checkout.session.completed missing customer".into())
        })?;
        let plan_str = session.client_reference_id.as_deref().ok_or_else(|| {
            AppError::BadRequest("checkout.session.completed missing client_reference_id".into())
        })?;
        let plan = Plan::from_str(plan_str)?;
        let user = User::find_by_stripe_customer_id(self.db.as_ref(), customer.as_str()).await?;
        let mut tx = self.db.begin().await?;
        User::set_role(&mut *tx, user.id, &plan.role()).await?;
        // Stripe now owns this plan's lifecycle, so any admin-granted expiry
        // date must go — otherwise the scheduler would downgrade a paying
        // customer on the old deadline.
        User::clear_plan_expiry(&mut *tx, user.id).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn downgrade_to_free(&self, customer: &CustomerId) -> Result<()> {
        let user = User::find_by_stripe_customer_id(self.db.as_ref(), customer.as_str()).await?;
        let mut tx = self.db.begin().await?;
        User::set_role(&mut *tx, user.id, &Role::Free).await?;
        User::clear_plan_expiry(&mut *tx, user.id).await?;
        tx.commit().await?;
        Ok(())
    }
}
