use crate::prelude::*;

/// A hand-typed email address that has passed a shape check and been
/// normalized: trimmed and lowercased, since sign-in matches the stored
/// address exactly and Google always hands us a lowercase one.
///
/// The check deliberately stays permissive — the goal is to catch typos like a
/// missing `@`, not to re-implement RFC 5322. Owning the address is proven
/// separately, by entering a code we email to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    /// The longest address SMTP allows (RFC 5321).
    pub const MAX_LEN: usize = 254;

    pub fn parse(raw: &str) -> Result<Self> {
        let email = raw.trim().to_lowercase();
        if !is_valid(&email) {
            return Err(AppError::BadRequest(
                "Enter a valid email address.".to_string(),
            ));
        }
        Ok(Email(email))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_valid(email: &str) -> bool {
    if email.len() > Email::MAX_LEN || email.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

impl Display for Email {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases() {
        assert_eq!(
            Email::parse("  Ada@Example.COM ").unwrap().as_str(),
            "ada@example.com"
        );
    }

    #[test]
    fn rejects_malformed_addresses() {
        for raw in [
            "",
            "ada",
            "@example.com",
            "ada@example",
            "ada@.example.com",
            "ada@example.com.",
            "ada@example..com",
            "a da@example.com",
            "ada@b@example.com",
        ] {
            assert!(Email::parse(raw).is_err(), "{raw} should be invalid");
        }
    }
}
