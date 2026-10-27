//! Outbound email through the `mailman` microservice (`MAILMAN_URL`,
//! `MAILMAN_API_TOKEN`).

use crate::domain::one_time_passcode;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::prelude::*;
use chrono::NaiveDate;
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
    send(&sign_in_code_request(
        recipient,
        first_name,
        code,
        verifying,
        &app_name(),
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

/// Warns someone that their admin-granted Professional access is about to
/// lapse. Sent from the daily scheduler tick a month, a week, and a day out.
/// Delivery failures are logged — a bounced reminder must not stop the tick
/// from recording that the reminder was attempted, or every later tick would
/// retry it forever.
pub async fn send_plan_expiry_reminder(
    recipient: &str,
    first_name: &str,
    days_remaining: i64,
    expires_on: NaiveDate,
) {
    let request = plan_expiry_reminder_request(
        recipient,
        first_name,
        days_remaining,
        expires_on,
        &app_name(),
        &app_origin(),
    );
    if let Err(error) = send(&request).await {
        error!(%error, email = %recipient, "could not send plan expiry reminder");
    }
}

fn plan_expiry_reminder_request(
    recipient: &str,
    first_name: &str,
    days_remaining: i64,
    expires_on: NaiveDate,
    app_name: &str,
    app_origin: &str,
) -> mailman::SendEmailRequest {
    let window = match days_remaining {
        d if d <= 0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        d => format!("in {d} days"),
    };
    let subject = format!("Your {app_name} plan ends {window}");
    let body = format!(
        "<h1>Your {} plan ends {}</h1>\
         <p>Hi {}, your Professional access ends on <strong>{}</strong>.</p>\
         <p>When it does, your account moves back to the Free plan. Nothing is \
            deleted — you just lose access to Professional features until you \
            upgrade again.</p>\
         <p style=\"margin: 32px 0; text-align: center;\">\
             <a href=\"{}/upgrade\" style=\"display: inline-block; font-size: 24px; font-weight: bold;\">\
                 Keep My Access\
             </a>\
         </p>\
         <p>If you already arranged an extension with us, you can ignore this email.</p>",
        escape_html(app_name),
        escape_html(&window),
        escape_html(first_name),
        expires_on.format("%B %-d, %Y"),
        escape_html(app_origin),
    );

    mailman::SendEmailRequest::new(subject, body, [recipient]).html(true)
}

/// Confirms that an admin-granted plan has lapsed and the account is back on
/// the Free plan. Sent by the same tick that performs the downgrade.
pub async fn send_plan_expired_notice(recipient: &str, first_name: &str) {
    let request = plan_expired_notice_request(recipient, first_name, &app_name(), &app_origin());
    if let Err(error) = send(&request).await {
        error!(%error, email = %recipient, "could not send plan expired notice");
    }
}

fn plan_expired_notice_request(
    recipient: &str,
    first_name: &str,
    app_name: &str,
    app_origin: &str,
) -> mailman::SendEmailRequest {
    let subject = format!("Your {app_name} plan has ended");
    let body = format!(
        "<h1>Your {} plan has ended</h1>\
         <p>Hi {}, your Professional access has ended and your account is back \
            on the Free plan.</p>\
         <p>Nothing was deleted. Upgrading again restores full access to \
            everything exactly as you left it.</p>\
         <p style=\"margin: 32px 0; text-align: center;\">\
             <a href=\"{}/upgrade\" style=\"display: inline-block; font-size: 24px; font-weight: bold;\">\
                 Upgrade Again\
             </a>\
         </p>",
        escape_html(app_name),
        escape_html(first_name),
        escape_html(app_origin),
    );

    mailman::SendEmailRequest::new(subject, body, [recipient]).html(true)
}

fn app_name() -> String {
    env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string())
}

fn app_origin() -> String {
    env::var("APP_ORIGIN")
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string()
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
    fn phrases_each_reminder_window_in_the_subject() {
        let expires_on = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let subject_for = |days| {
            plan_expiry_reminder_request(
                "someone@example.com",
                "Sam",
                days,
                expires_on,
                "Bibby",
                "https://bibby.example",
            )
            .subject
        };

        assert_eq!(subject_for(30), "Your Bibby plan ends in 30 days");
        assert_eq!(subject_for(7), "Your Bibby plan ends in 7 days");
        assert_eq!(subject_for(1), "Your Bibby plan ends tomorrow");
        assert_eq!(subject_for(0), "Your Bibby plan ends today");
    }

    #[test]
    fn builds_plan_expiry_emails() {
        let reminder = plan_expiry_reminder_request(
            "someone@example.com",
            "Sam & Co.",
            7,
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
            "Bibby",
            "https://bibby.example",
        );
        assert_eq!(reminder.to, ["someone@example.com"]);
        assert!(reminder.body.contains("Hi Sam &amp; Co."));
        assert!(reminder.body.contains("September 30, 2026"));
        assert!(
            reminder
                .body
                .contains("href=\"https://bibby.example/upgrade\"")
        );

        let notice = plan_expired_notice_request(
            "someone@example.com",
            "Sam",
            "Bibby",
            "https://bibby.example",
        );
        assert_eq!(notice.subject, "Your Bibby plan has ended");
        assert!(notice.body.contains("back on the Free plan"));
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
