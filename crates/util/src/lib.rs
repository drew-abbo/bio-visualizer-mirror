//! This crate contains useful utilities that will be shared between different
//! parts of the project.

#[cfg(feature = "cast_slice")]
pub mod cast_slice;
#[cfg(feature = "channels")]
pub mod channels;
#[cfg(feature = "crash_reporting")]
pub mod crash_reporting;
#[cfg(feature = "debug_log")]
pub mod debug_log;
#[cfg(feature = "drop_join_thread")]
pub mod drop_join_thread;
#[cfg(feature = "fuzzy_search")]
pub mod fuzzy_search;
#[cfg(feature = "gcd")]
pub mod gcd;
#[cfg(feature = "local_data")]
pub mod local_data;
#[cfg(feature = "read_write_at")]
pub mod read_write_at;
#[cfg(feature = "rolling_avg")]
pub mod rolling_avg;
#[cfg(feature = "saved_file")]
pub mod saved_file;
#[cfg(feature = "stop_signals")]
pub mod stop_signals;
#[cfg(feature = "strn")]
pub mod strn;
#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "uid")]
pub mod uid;
#[cfg(feature = "version")]
pub mod version;
