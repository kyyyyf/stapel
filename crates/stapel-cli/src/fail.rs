//! A failure that carries its exit code (STP-18): record checks exit 2, grant errors exit 1.

use std::process::ExitCode;

pub struct Fail {
    pub code: u8,
    pub message: String,
}

impl Fail {
    /// A refusal of a record check: exit 2.
    pub fn refused(message: impl Into<String>) -> Fail {
        Fail {
            code: 2,
            message: message.into(),
        }
    }
}

/// Prints the failure as `stapel: <reason>` on stderr and returns its exit code.
pub fn finish(result: Result<(), Fail>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            warn!("stapel: {}", f.message);
            ExitCode::from(f.code)
        }
    }
}
