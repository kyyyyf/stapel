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

    /// A grant error, an agent shell or any other failure of the run itself: exit 1.
    pub fn failed(message: impl Into<String>) -> Fail {
        Fail {
            code: 1,
            message: message.into(),
        }
    }
}

/// The refusal of a record command in an agent's shell without a grant (STP-18 AC-12).
pub const AGENT_SHELL: &str = "only a person writes review records and ledger records: this \
     shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command \
     as one plain command so that Claude Code shows the permission dialog, or, from a Zed task \
     started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT";

/// The text of a grant that matches no dialog of a record command (STP-18 AC-14).
pub const MISMATCH: &str = "the grant does not match: the records, the ticket or the stapel \
     build changed since the dialog, or the grant was used already; ask the agent to run the \
     command again";

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
