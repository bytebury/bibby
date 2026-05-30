pub mod announcement;
pub mod blog;
pub mod country;
pub mod region;
pub mod user;
pub mod value_objects;

use crate::prelude::*;

/// Resource-level authorization. Implement `can_read` / `can_write` on the
/// principal type (e.g. `User`) for each entity it should be able to act on.
pub trait Access<T> {
    fn can_read(&self, _entity: &T) -> bool {
        true
    }
    fn can_write(&self, _entity: &T) -> bool {
        false
    }
}

/// Boundary validation for inbound form/request bodies. Return an
/// `AppError::BadRequest` describing the first problem; its message is what the
/// user sees (a 400 body the frontend surfaces as a toast). Prefer the
/// `ValidatedForm<T>` extractor over calling this by hand so a handler can't
/// forget to run it.
pub trait Validate {
    fn validate(&self) -> Result<()>;
}
