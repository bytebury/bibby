use crate::domain::value_objects::email::Email;
use crate::prelude::*;

/// The addresses in `ADMIN_EMAILS` (comma-separated, whitespace trimmed).
/// An account with one of them becomes an admin once it proves it owns the
/// address: a verified Google sign-in or an emailed code. Leaving the list
/// doesn't demote anyone, since admins can also be made from the user page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminEmails(Vec<Email>);

impl AdminEmails {
    /// Reads `ADMIN_EMAILS`. Without it a fresh deploy would have no way to
    /// get an admin, so a missing, empty or malformed list stops startup.
    pub fn from_env() -> Self {
        let raw = env::var("ADMIN_EMAILS").unwrap_or_default();
        Self::parse(&raw).unwrap_or_else(|e| panic!("ADMIN_EMAILS: {e}"))
    }

    pub fn parse(raw: &str) -> std::result::Result<Self, String> {
        let emails = raw
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                Email::parse(entry).map_err(|_| format!("{entry:?} is not an email address"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if emails.is_empty() {
            return Err("set it to a comma-separated list of admin email addresses".to_string());
        }
        Ok(Self(emails))
    }

    pub fn contains(&self, email: &str) -> bool {
        let email = email.trim().to_lowercase();
        self.0.iter().any(|admin| admin.as_str() == email)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases_each_entry() {
        let admins = AdminEmails::parse(" Ada@Example.com ,grace@example.com,, ").unwrap();
        assert!(admins.contains("ada@example.com"));
        assert!(admins.contains("GRACE@example.com"));
        assert!(!admins.contains("eve@example.com"));
    }

    #[test]
    fn rejects_an_empty_list() {
        assert!(AdminEmails::parse("").is_err());
        assert!(AdminEmails::parse(" , ").is_err());
    }

    #[test]
    fn rejects_a_malformed_entry() {
        assert!(AdminEmails::parse("ada@example.com, not-an-email").is_err());
    }
}
