pub mod announcement;
pub mod blog;
pub mod country;
pub mod region;
pub mod user;
pub mod value_objects;

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
