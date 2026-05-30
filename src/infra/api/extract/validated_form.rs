use crate::domain::Validate;
use crate::prelude::*;
use axum::extract::{FromRequest, Request};
use axum_extra::extract::Form;

/// `axum_extra::extract::Form<T>` that runs `T::validate` after deserializing,
/// so the form's invariants are enforced at the HTTP boundary and a handler
/// can't forget to check them. Either failure becomes an `AppError::BadRequest`
/// whose message the frontend surfaces to the user.
pub struct ValidatedForm<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedForm<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self> {
        let Form(value) = Form::<T>::from_request(req, state).await.map_err(|_| {
            AppError::BadRequest(
                "We couldn't read that form. Please check your input and try again.".into(),
            )
        })?;
        value.validate()?;
        Ok(ValidatedForm(value))
    }
}
