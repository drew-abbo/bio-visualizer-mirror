//! Whether or not [crate::debug_log_error] should panic.

#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether panicking on errors is enabled or not (see
/// [crate::debug_log_error]).
///
/// [super::enabled] must be true for this to have any effect. This is enabled
/// by default.
#[cfg(debug_assertions)]
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

/// Disable panicking on errors (see [crate::debug_log_error]).
///
/// Nothing happens if [enabled] is false, otherwise it's enabled by default.
pub fn disable() {
    #[cfg(debug_assertions)]
    ENABLED.store(false, Ordering::Relaxed);
}

/// Enable panicking on errors (see [crate::debug_log_error]).
///
/// Nothing happens if [enabled] is false, otherwise it's enabled by default.
/// Trying to manually enable logging when `cfg!(debug_assertions)` is false
/// will result in the program panicking.
pub fn enable() {
    #[cfg(not(debug_assertions))]
    panic!("Panicking on errors cannot be enabled because debug logging cannot be enabled.");

    #[cfg(debug_assertions)]
    ENABLED.store(true, Ordering::Relaxed);
}

#[cfg(debug_assertions)]
static ENABLED: AtomicBool = AtomicBool::new(true);
