//! Defines [eprint_and_exit] and [eprintln_and_exit].

/// The equivalent to calling [eprint], then calling [std::process::exit] with
/// with an exit code of `1`.
///
/// Useful for exiting gracefully with an error message.
#[macro_export]
macro_rules! eprint_and_exit {
    ($($arg:tt)*) => {{
        eprint!($($arg)*);
        ::std::process::exit(1);
    }};
}

/// The equivalent to calling [eprintln], then calling [std::process::exit] with
/// with an exit code of `1`.
///
/// Useful for exiting gracefully with an error message.
#[macro_export]
macro_rules! eprintln_and_exit {
    ($($arg:tt)*) => {{
        eprintln!($($arg)*);
        ::std::process::exit(1);
    }};
}
