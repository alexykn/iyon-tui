use napi::{Error, Status};

/// Stable error categories exposed by the native addon surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeError {
    InvalidInput,
    Internal,
    Cancelled,
    Closed,
}

impl NativeError {
    pub fn invalid_input(message: impl Into<String>) -> Error {
        Self::coded(
            Status::InvalidArg,
            "ION_INVALID_INPUT",
            format!("invalid input: {}", message.into()),
        )
    }

    /// Preserves the stable content-plane error family carried by the native
    /// core instead of collapsing every lifecycle failure into
    /// `ION_INVALID_INPUT`. The core diagnostics use `CODE: detail`; only the
    /// known control codes are promoted to Node-API codes.
    pub fn content(error: impl std::fmt::Display) -> Error {
        let message = error.to_string();
        let code = message
            .split_once(':')
            .map(|(code, _)| code)
            .filter(|code| is_content_code(code.trim()))
            .map(|code| format!("ION_{}", code.trim()))
            .unwrap_or_else(|| "ION_INTERNAL".to_owned());
        Error::new(Status::InvalidArg, format!("{code}: {message}"))
    }

    pub fn internal(message: impl Into<String>) -> Error {
        Self::coded(Status::GenericFailure, "ION_INTERNAL", message)
    }

    pub fn cancelled() -> Error {
        Self::coded(Status::Cancelled, "ION_CANCELLED", "operation cancelled")
    }

    pub fn closed() -> Error {
        Self::coded(Status::Closing, "ION_CLOSED", "native operation is closed")
    }

    pub fn coded(status: Status, code: &str, message: impl Into<String>) -> Error {
        Error::new(status, format!("{code}: {}", message.into()))
    }
}

fn is_content_code(code: &str) -> bool {
    matches!(
        code,
        "CONTENT_FAMILY_MISMATCH"
            | "CONNECTOR_DISPOSING"
            | "CONNECTOR_NOT_MEMBER"
            | "CONNECTOR_DISPOSED"
            | "DUPLICATE_CONTENT_PORT_ATTACHMENT"
            | "HOST_DISPOSED"
            | "INTERNAL_INVARIANT"
            | "INVALID_ARGUMENT"
            | "INVALID_FUNNEL"
            | "PORT_IN_USE"
            | "PORT_MOUNTED"
            | "PORT_DISPOSED"
            | "PROJECTION_FAILED"
            | "SOURCE_IN_USE"
            | "SOURCE_DISPOSED"
            | "SOURCE_SEALED"
            | "SOURCE_ALREADY_SEALED"
            | "SOURCE_RETENTION_OVERFLOW"
            | "STALE_SOURCE"
            | "INVALID_UTF8"
            | "INVALID_RANGE"
            | "UNKNOWN_ANNOTATION_KIND"
            | "INVALID_ANNOTATION_PAYLOAD"
            | "LIMIT_EXCEEDED"
            | "PAYLOAD_TOO_LARGE"
            | "RUNTIME_POISONED"
            | "STALE_HANDLE"
            | "UNSUPPORTED_CONTENT_PORT_ATTACHMENT"
            | "WRONG_ENVIRONMENT"
            | "WRONG_HOST"
    )
}
