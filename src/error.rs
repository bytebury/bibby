use askama_web::__askama_web_impl::axum_core_0_5::{IntoResponse, Response};
use axum::http::StatusCode;
use oauth2::reqwest;
use std::fmt::Display;
use std::net::AddrParseError;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Forbidden(String),
    Unauthorized,
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if matches!(err, sqlx::Error::RowNotFound) {
            return AppError::NotFound("Not found".into());
        }
        match err.as_database_error().and_then(|e| e.code()) {
            Some(code) if matches!(code.as_ref(), "2067" | "1555" | "23505") => {
                AppError::Conflict("This record already exists.".into())
            }
            _ => AppError::Internal(err.to_string()),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(format!("IO error: {}", err))
    }
}

impl From<AddrParseError> for AppError {
    fn from(err: AddrParseError) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<geodude::Error> for AppError {
    fn from(err: geodude::Error) -> Self {
        Self::Internal(format!("geodude lookup failed: {err}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            Self::NotFound(_) | Self::Forbidden(_) | Self::Unauthorized => {
                tracing::debug!("{:?}", self)
            }
            _ => tracing::error!("{:?}", self),
        }
        match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg).into_response(),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg).into_response(),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg).into_response(),
            Self::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
            }
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(message) => f.write_str(message),
            Self::NotFound(message) => f.write_str(message),
            Self::Conflict(message) => f.write_str(message),
            Self::Forbidden(message) => f.write_str(message),
            Self::Internal(message) => f.write_str(message),
            Self::Unauthorized => f.write_str("Unauthorized"),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
