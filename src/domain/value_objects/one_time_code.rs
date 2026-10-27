use crate::domain::value_objects::email::Email;
use crate::prelude::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const ONE_TIME_CODE_LEN: usize = 6;

/// A six-digit code we email to verify an address or sign someone in.
///
/// The plaintext only ever lives in the email and the form it is typed into.
/// What we persist is `digest`: an HMAC keyed with `JWT_SECRET` and bound to
/// the recipient's address, so a leaked row can't be brute-forced offline and
/// a code minted for one address is useless for another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneTimeCode(String);

impl OneTimeCode {
    /// Cryptographically random, zero-padded six-digit code.
    pub fn generate() -> Self {
        use rand::RngExt;
        let number = rand::rng().random_range(0..1_000_000u32);
        OneTimeCode(format!("{number:0width$}", width = ONE_TIME_CODE_LEN))
    }

    /// Accepts what people actually paste — `123456`, `123 456`, `123-456` —
    /// and rejects anything that isn't six digits once spacing is removed.
    pub fn parse(raw: &str) -> Result<Self> {
        let code: String = raw
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        if code.len() != ONE_TIME_CODE_LEN || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "Enter the 6-digit code from your email.".to_string(),
            ));
        }
        Ok(OneTimeCode(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self, email: &Email) -> Vec<u8> {
        self.mac(&secret(), email).finalize().into_bytes().to_vec()
    }

    /// Constant-time comparison against a stored `digest`.
    pub fn matches(&self, email: &Email, digest: &[u8]) -> bool {
        self.mac(&secret(), email).verify_slice(digest).is_ok()
    }

    fn mac(&self, key: &[u8], email: &Email) -> Hmac<Sha256> {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(email.as_str().as_bytes());
        mac.update(b":");
        mac.update(self.0.as_bytes());
        mac
    }
}

fn secret() -> Vec<u8> {
    env::var("JWT_SECRET")
        .expect("JWT_SECRET is not present")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_six_digits() {
        for _ in 0..100 {
            let code = OneTimeCode::generate();
            assert_eq!(code.as_str().len(), ONE_TIME_CODE_LEN);
            assert!(code.as_str().chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn parses_pasted_codes() {
        for raw in ["123456", " 123456 ", "123 456", "123-456"] {
            assert_eq!(OneTimeCode::parse(raw).unwrap().as_str(), "123456");
        }
    }

    #[test]
    fn rejects_malformed_codes() {
        for raw in ["", "12345", "1234567", "12345a", "abcdef"] {
            assert!(OneTimeCode::parse(raw).is_err(), "{raw} should be invalid");
        }
    }

    #[test]
    fn digests_are_bound_to_code_email_and_key() {
        let email = Email::parse("someone@example.com").unwrap();
        let other = Email::parse("other@example.com").unwrap();
        let code = OneTimeCode::parse("123456").unwrap();
        let digest = code.mac(b"key", &email).finalize().into_bytes();

        assert!(code.mac(b"key", &email).verify_slice(&digest).is_ok());
        assert!(
            OneTimeCode::parse("654321")
                .unwrap()
                .mac(b"key", &email)
                .verify_slice(&digest)
                .is_err()
        );
        assert!(code.mac(b"key", &other).verify_slice(&digest).is_err());
        assert!(code.mac(b"other", &email).verify_slice(&digest).is_err());
    }
}
