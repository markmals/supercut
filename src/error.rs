use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value};

/// Stable error categories; agents branch on these, not on message text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// The command line or an input file is malformed.
    InvalidArgument,
    /// A named show, episode, plan or file does not exist.
    NotFound,
    /// Something must happen first (sync, download, transcribe, label...).
    FailedPrecondition,
    /// The operation would overwrite something; pass `--force` to allow it.
    Conflict,
    /// Credentials are missing or rejected.
    Unauthenticated,
    /// A remote service throttled us; retry later.
    RateLimited,
    /// A network or remote service failure that may succeed on retry.
    Unavailable,
    /// ffmpeg, mlx_whisper or another external program failed.
    ExternalTool,
    /// Anything else.
    Internal,
}

impl ErrorCode {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::InvalidArgument => 2,
            Self::NotFound => 3,
            Self::FailedPrecondition | Self::Conflict => 4,
            Self::ExternalTool => 5,
            Self::Unauthenticated => 6,
            Self::RateLimited | Self::Unavailable => 7,
            Self::Internal => 1,
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, Self::RateLimited | Self::Unavailable)
    }
}

/// An error with a stable code, an optional next step and structured details. Wrap it in
/// `anyhow::Error` like any other error; `main` finds it anywhere in the context chain.
#[derive(Debug)]
pub struct CliError {
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
    pub details: Map<String, Value>,
}

impl CliError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), hint: None, details: Map::new() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn precondition(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::FailedPrecondition, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    /// The command (or action) that resolves the error.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// The JSON written to stderr when a command fails.
#[derive(Serialize)]
pub struct ErrorReport<'a> {
    pub code: ErrorCode,
    /// The full context chain, outermost first.
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<&'a str>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub details: Map<String, Value>,
}

impl<'a> ErrorReport<'a> {
    pub fn from_anyhow(error: &'a anyhow::Error) -> Self {
        let tagged = error.chain().find_map(|cause| cause.downcast_ref::<CliError>());
        let code = tagged.map_or(ErrorCode::Internal, |tagged| tagged.code);
        Self {
            code,
            message: format!("{error:#}"),
            retryable: code.retryable(),
            hint: tagged.and_then(|tagged| tagged.hint.as_deref()),
            details: tagged.map(|tagged| tagged.details.clone()).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_and_hint_survive_added_context() {
        let error = anyhow::Error::from(
            CliError::not_found("no episode `s10e99`").hint("supercut episodes list x"),
        )
        .context("loading plan");
        let report = ErrorReport::from_anyhow(&error);
        assert_eq!(report.code, ErrorCode::NotFound);
        assert_eq!(report.message, "loading plan: no episode `s10e99`");
        assert_eq!(report.hint, Some("supercut episodes list x"));
        assert!(!report.retryable);
    }
}
