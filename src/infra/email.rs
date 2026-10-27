//! Outbound email through the `mailman` microservice (`MAILMAN_URL`,
//! `MAILMAN_API_TOKEN`).

use crate::domain::one_time_passcode;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::prelude::*;
use std::time::Duration;

/// Emails a sign-in code. The caller needs to know whether this landed: an
/// undelivered code should not count against the resend cooldown.
///
/// The first code an email sign-up receives doubles as proof they own the
/// address, so `verifying` swaps the copy to say so.
pub async fn send_sign_in_code(
    recipient: &Email,
    first_name: &str,
    code: &OneTimeCode,
    verifying: bool,
) -> Result<()> {
    let app_name = env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string());
    send(&sign_in_code_request(
        recipient, first_name, code, verifying, &app_name,
    ))
    .await
}

fn sign_in_code_request(
    recipient: &Email,
    first_name: &str,
    code: &OneTimeCode,
    verifying: bool,
    app_name: &str,
) -> mailman::SendEmailRequest {
    let (subject, heading, purpose, ignore) = if verifying {
        (
            format!("Verify your {app_name} email"),
            "Verify your email",
            "use this code to verify your email and finish creating your account on",
            "If you didn't sign up, you can ignore this email. The account won't be \
             usable without this code.",
        )
    } else {
        (
            format!("Your {app_name} sign-in code"),
            "Your sign-in code",
            "use this code to sign in to",
            "If you didn't try to sign in, you can ignore this email. Nobody can \
             get into your account without this code.",
        )
    };
    let body = format!(
        "<h1>{}</h1>\
         <p>Hi {}, {} {}:</p>\
         <p style=\"margin: 32px 0; text-align: center; font-size: 36px; font-weight: bold; letter-spacing: 8px; font-family: monospace;\">\
             {}\
         </p>\
         <p>This code expires in {} minutes and can only be used once.</p>\
         <p>{}</p>",
        heading,
        escape_html(first_name),
        purpose,
        escape_html(app_name),
        code.as_str(),
        one_time_passcode::TTL_MINUTES,
        ignore,
    );

    mailman::SendEmailRequest::new(subject, body, [recipient.as_str()]).html(true)
}

async fn send(request: &mailman::SendEmailRequest) -> Result<()> {
    let url = env::var("MAILMAN_URL")
        .map_err(|_| AppError::Internal("MAILMAN_URL must be set".to_string()))?;
    let token = env::var("MAILMAN_API_TOKEN").unwrap_or_default();
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let client = mailman::Client::with_http(http, url, token)
        .map_err(|error| AppError::Internal(error.to_string()))?;

    client
        .send(request)
        .await
        .map(|_| ())
        .map_err(|error| AppError::Internal(format!("Mailman delivery failed: {error}")))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_for_html_email_content_and_attributes() {
        assert_eq!(
            escape_html(r#"<Acme & "Co">'s"#),
            "&lt;Acme &amp; &quot;Co&quot;&gt;&#39;s"
        );
    }

    #[test]
    fn builds_sign_in_code_email() {
        let recipient = Email::parse("someone@example.com").unwrap();
        let code = OneTimeCode::parse("042137").unwrap();
        let request = sign_in_code_request(&recipient, "Sam & Co.", &code, false, "Bibby");

        assert_eq!(request.subject, "Your Bibby sign-in code");
        assert_eq!(request.to, ["someone@example.com"]);
        assert!(request.html);
        assert!(request.body.contains("Hi Sam &amp; Co."));
        assert!(request.body.contains("use this code to sign in to Bibby"));
        assert!(request.body.contains("042137"));
        assert!(request.body.contains("expires in 10 minutes"));
    }

    #[test]
    fn builds_verification_code_email() {
        let recipient = Email::parse("someone@example.com").unwrap();
        let code = OneTimeCode::parse("042137").unwrap();
        let request = sign_in_code_request(&recipient, "Sam", &code, true, "Bibby");

        assert_eq!(request.subject, "Verify your Bibby email");
        assert!(request.body.contains("verify your email"));
        assert!(request.body.contains("042137"));
    }
}
