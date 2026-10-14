//! The only place where the binary writes to standard output and standard error.
//!
//! A closed pipe makes a write fail with `BrokenPipe`. The macros drop that error, so a command
//! that prints between two writes still finishes its work and keeps its exit code. Any other write
//! error panics with the text `println!` uses. No output handle leaves this module.

use std::io::Write;

fn finish(result: std::io::Result<()>, name: &str) {
    match result {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => panic!("failed printing to {name}: {e}"),
    }
}

#[doc(hidden)]
pub fn stdout(args: std::fmt::Arguments<'_>, newline: bool) {
    let mut out = std::io::stdout().lock();
    let result = out.write_fmt(args);
    let result = match (result, newline) {
        (Ok(()), true) => out.write_all(b"\n"),
        (result, _) => result,
    };
    finish(result, "stdout");
}

#[doc(hidden)]
pub fn stderr(args: std::fmt::Arguments<'_>) {
    let mut err = std::io::stderr().lock();
    let result = err.write_fmt(args).and_then(|()| err.write_all(b"\n"));
    finish(result, "stderr");
}

/// Flushes standard output: a closed pipe is not an error, any other error is returned.
pub fn flush() -> std::io::Result<()> {
    match std::io::stdout().flush() {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

/// A line to standard output.
macro_rules! say {
    () => {
        $crate::out::stdout(format_args!(""), true)
    };
    ($($arg:tt)*) => {
        $crate::out::stdout(format_args!($($arg)*), true)
    };
}

/// Text to standard output, without a newline.
macro_rules! say_raw {
    ($($arg:tt)*) => {
        $crate::out::stdout(format_args!($($arg)*), false)
    };
}

/// A line to standard error.
macro_rules! warn {
    () => {
        $crate::out::stderr(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::out::stderr(format_args!($($arg)*))
    };
}
