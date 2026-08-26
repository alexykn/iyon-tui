use napi::{Error, Status};

/// Stable error categories exposed by the smoke bridge.
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
