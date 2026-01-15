//! Contains tools for debug-mode logging.
//!
//! Logging cannot be enabled when `cfg!(debug_assertions)` is false, otherwise
//! it's enabled by default.

pub mod panic_on_errors;

use std::panic::Location;
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicBool, Ordering};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Log some info to stdout if both `cfg!(debug_assertions)` and [enabled] are
/// true.
#[macro_export]
macro_rules! debug_log_info {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        if $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stdout());
            let (blue, magenta, reset_color) = if is_terminal {
                ("\x1b[34m", "\x1b[35m", "\x1b[0m")
            } else {
                ("", "", "")
            };

            let where_and_when = $crate::debug_log::where_and_when(blue, reset_color);

            ::std::println!(
                "{blue}DEBUG LOG{reset_color} [{magenta}INFO{reset_color}]: {}\n{where_and_when}",
                format!($($arg)*),
            );
        }
    }};
}

/// Log a warning to stderr if both `cfg!(debug_assertions)` and [enabled] are
/// true.
#[macro_export]
macro_rules! debug_log_warning {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        if $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stderr());
            let (blue, yellow, reset_color) = if is_terminal {
                ("\x1b[34m", "\x1b[33m", "\x1b[0m")
            } else {
                ("", "", "")
            };

            let where_and_when = $crate::debug_log::where_and_when(blue, reset_color);

            ::std::eprintln!(
                "{blue}DEBUG LOG{reset_color} [{yellow}WARNING{reset_color}]: {}\n{where_and_when}",
                format!($($arg)*),
            );
        }
    }};
}

/// Log an error to stderr if both `cfg!(debug_assertions)` and [enabled] are
/// true.
#[macro_export]
macro_rules! debug_log_error {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        if $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stderr());
            let (blue, red, reset_color) = if is_terminal {
                ("\x1b[34m", "\x1b[31m", "\x1b[0m")
            } else {
                ("", "", "")
            };

            let where_and_when = $crate::debug_log::where_and_when(blue, reset_color);

            ::std::eprintln!(
                "{blue}DEBUG LOG{reset_color} [{red}ERROR{reset_color}]: {}\n{where_and_when}",
                format!($($arg)*),
            );

            if $crate::debug_log::panic_on_errors::enabled() {
                panic!("Panicking on error logging enabled.");
            }
        }
    }};
}

/// Whether logging is enabled or not.
///
/// Logging cannot be enabled when `cfg!(debug_assertions)` is false, otherwise
/// it's enabled by default.
#[inline(always)]
pub fn enabled() -> bool {
    #[cfg(not(debug_assertions))]
    #[inline(always)]
    fn enabled_impl() -> bool {
        false
    }

    #[cfg(debug_assertions)]
    #[inline(always)]
    fn enabled_impl() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    enabled_impl()
}

/// Disable logging.
///
/// Logging cannot be enabled when `cfg!(debug_assertions)` is false, otherwise
/// it's enabled by default.
#[inline(always)]
pub fn disable() {
    #[cfg(debug_assertions)]
    ENABLED.store(false, Ordering::Relaxed);
}

/// Enable logging.
///
/// Logging cannot be enabled when `cfg!(debug_assertions)` is false, otherwise
/// it's enabled by default. Trying to manually enable logging when
/// `cfg!(debug_assertions)` is false will result in the program panicking.
#[inline(always)]
pub fn enable() {
    #[cfg(not(debug_assertions))]
    panic!("Debug logging cannot be enabled.");

    #[cfg(debug_assertions)]
    ENABLED.store(true, Ordering::Relaxed);
}

/// The location of the caller, the time this was called, and the executable
/// (argv), all as strings.
///
/// This function gets called by the debug log macros (e.g. [debug_log_info])
/// and generally shouldn't be called directly.
#[track_caller]
pub fn where_and_when(color: &str, reset_color: &str) -> String {
    let now = OffsetDateTime::now_utc();

    let loc = Location::caller();
    let where_ = format!("{}:{}:{}", loc.file(), loc.line(), loc.column());

    let when = now
        .format(&Rfc3339)
        .unwrap_or_else(|e| format!("Unknown time: {e}"));

    let exec = std::env::args().collect::<Vec<_>>().join(" ");

    format!("\tWhere: {color}{where_}{reset_color}\n")
        + format!("\tTime:  {color}{when}{reset_color}\n").as_str()
        + format!("\tExec.: {color}{exec}{reset_color}").as_str()
}

#[cfg(debug_assertions)]
static ENABLED: AtomicBool = AtomicBool::new(true);
