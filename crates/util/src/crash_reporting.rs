//! This module contains code that reports crashes when the program is in debug
//! mode.

#[cfg(debug_assertions)]
mod crash_reporting_impl;

/// Initializes crash reporting if `debug_assertions` are enabled. If they are
/// not, this is a no-op.
#[cfg_attr(not(debug_assertions), inline(always))]
pub fn init() {
    #[cfg(debug_assertions)]
    crash_reporting_impl::init_impl();
}
