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
        if ::std::cfg!(debug_assertions) && $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stdout());
            let (blue, magenta, reset_color) = is_terminal
                .then_some(("\x1b[34m", "\x1b[35m", "\x1b[0m"))
                .unwrap_or_default();

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
        if ::std::cfg!(debug_assertions) && $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stderr());
            let (blue, yellow, reset_color) = is_terminal
                .then_some(("\x1b[34m", "\x1b[33m", "\x1b[0m"))
                .unwrap_or_default();

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
        if ::std::cfg!(debug_assertions) && $crate::debug_log::enabled() {
            let is_terminal = ::std::io::IsTerminal::is_terminal(&::std::io::stderr());
            let (blue, red, reset_color) = is_terminal
                .then_some(("\x1b[34m", "\x1b[31m", "\x1b[0m"))
                .unwrap_or_default();

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
#[doc(hidden)]
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

#[cfg(test)]
mod decision_coverage_tests {
    use super::*;
    use crate::debug_log::panic_on_errors;
    use std::sync::{Mutex, MutexGuard};

    /// Global serialization lock for tests that touch shared atomic state.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that restores known-good state when dropped (even on panic).
    struct StateGuard<'a> {
        _lock: MutexGuard<'a, ()>,
    }

    impl<'a> StateGuard<'a> {
        fn acquire() -> Self {
            let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // Restore defaults before each test
            enable();
            panic_on_errors::disable();
            Self { _lock: lock }
        }
    }

    impl Drop for StateGuard<'_> {
        fn drop(&mut self) {
            // Restore defaults after each test (even on panic)
            enable();
            panic_on_errors::disable();
        }
    }

    // --- enabled() ---

    #[test]
    fn enabled_returns_true_by_default() {
        let _g = StateGuard::acquire();
        enable();
        assert!(enabled());
    }

    #[test]
    fn enabled_returns_false_after_disable() {
        let _g = StateGuard::acquire();
        disable();
        assert!(!enabled());
    }

    // --- disable() ---

    #[test]
    fn disable_sets_enabled_to_false() {
        let _g = StateGuard::acquire();
        enable();
        disable();
        assert!(!enabled());
    }

    #[test]
    fn disable_is_idempotent() {
        let _g = StateGuard::acquire();
        disable();
        disable();
        assert!(!enabled());
    }

    // --- enable() ---

    #[test]
    fn enable_sets_enabled_to_true() {
        let _g = StateGuard::acquire();
        disable();
        enable();
        assert!(enabled());
    }

    #[test]
    fn enable_is_idempotent() {
        let _g = StateGuard::acquire();
        enable();
        enable();
        assert!(enabled());
    }

    // --- where_and_when() ---

    #[test]
    fn where_and_when_with_no_color_codes_contains_expected_sections() {
        let result = where_and_when("", "");
        assert!(result.contains("Where:"));
        assert!(result.contains("Time:"));
        assert!(result.contains("Exec.:"));
    }

    #[test]
    fn where_and_when_with_color_codes_contains_expected_sections() {
        let result = where_and_when("\x1b[34m", "\x1b[0m");
        assert!(result.contains("Where:"));
        assert!(result.contains("Time:"));
        assert!(result.contains("Exec.:"));
        assert!(result.contains("\x1b[34m"));
        assert!(result.contains("\x1b[0m"));
    }

    #[test]
    fn where_and_when_contains_caller_file() {
        let result = where_and_when("", "");
        assert!(result.contains(file!()));
    }

    #[test]
    fn where_and_when_time_is_rfc3339_formatted() {
        let result = where_and_when("", "");
        let time_line = result
            .lines()
            .find(|l| l.contains("Time:"))
            .expect("Time line missing");
        assert!(time_line.contains('T'), "Expected RFC3339 'T' separator");
    }

    #[test]
    fn where_and_when_exec_line_is_present() {
        let result = where_and_when("", "");
        let exec_line = result
            .lines()
            .find(|l| l.contains("Exec.:"))
            .expect("Exec. line missing");
        assert!(!exec_line.trim_start_matches("\tExec.:").trim().is_empty());
    }

    // --- debug_log_info! ---

    #[test]
    fn debug_log_info_does_not_panic_when_enabled() {
        let _g = StateGuard::acquire();
        enable();
        debug_log_info!("test info message {}", 42);
    }

    #[test]
    fn debug_log_info_does_not_panic_when_disabled() {
        let _g = StateGuard::acquire();
        disable();
        debug_log_info!("this should be skipped");
    }

    // --- debug_log_warning! ---

    #[test]
    fn debug_log_warning_does_not_panic_when_enabled() {
        let _g = StateGuard::acquire();
        enable();
        debug_log_warning!("test warning message {}", 42);
    }

    #[test]
    fn debug_log_warning_does_not_panic_when_disabled() {
        let _g = StateGuard::acquire();
        disable();
        debug_log_warning!("this should be skipped");
    }

    // --- debug_log_error! ---

    #[test]
    fn debug_log_error_does_not_panic_when_disabled() {
        let _g = StateGuard::acquire();
        disable();
        debug_log_error!("this should be skipped");
    }

    #[test]
    fn debug_log_error_does_not_panic_when_panic_on_errors_disabled() {
        let _g = StateGuard::acquire();
        enable();
        panic_on_errors::disable(); // false branch of inner decision
        debug_log_error!("test error message {}", 42);
    }

    #[test]
    #[should_panic(expected = "Panicking on error logging enabled.")]
    fn debug_log_error_panics_when_panic_on_errors_enabled() {
        let _g = StateGuard::acquire();
        enable();
        panic_on_errors::enable();
        debug_log_error!("this should panic");
    }
}
