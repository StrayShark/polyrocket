//! L4 — Application-wide error type.
//!
//! Single error enum that all layers below return. Serializes to a
//! human-readable string for IPC (frontend gets `err.message`).
//!
//! 8 stable error codes are handled at L3 (LLM clients) by mapping
//! HTTP/SDK errors into a string. This enum does NOT model those
//! categories — it only cares about the transport layer (Db, Http,
//! Io, Serde, Keyring, plus Invalid/NotFound/Internal for app logic).

use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("keyring error: {0}")]
    Keyring(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keyring(e.to_string())
    }
}
