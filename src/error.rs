//! Error types for io_uring operations

use std::fmt;
use std::io;

/// Result type for io_uring operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when working with io_uring
#[derive(Debug)]
pub enum Error {
    /// I/O error from the system
    Io(io::Error),

    /// io_uring setup failed
    Setup(io::Error),

    /// Submission queue is full
    SubmissionQueueFull,

    /// No completion queue entries available
    CompletionQueueEmpty,

    /// Invalid operation or parameter
    InvalidOperation(String),

    /// Feature not supported by the kernel
    NotSupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Setup(e) => write!(f, "io_uring setup failed: {}", e),
            Error::SubmissionQueueFull => write!(f, "submission queue is full"),
            Error::CompletionQueueEmpty => write!(f, "no completion queue entries available"),
            Error::InvalidOperation(msg) => write!(f, "invalid operation: {}", msg),
            Error::NotSupported(msg) => write!(f, "feature not supported: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) | Error::Setup(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// Convert a negative return code to an io::Error
pub(crate) fn from_ret_code(ret: i32) -> io::Error {
    io::Error::from_raw_os_error(-ret)
}

/// Check a return code and convert to Result
pub(crate) fn check_ret(ret: i32) -> io::Result<i32> {
    if ret < 0 {
        Err(from_ret_code(ret))
    } else {
        Ok(ret)
    }
}
