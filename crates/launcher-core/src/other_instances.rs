//! This module contains tools for ensuring there is only ever one main
//! instance.
//!
//! "OI" is short for "Other Instance".

mod instance_lock;
mod oi_messager;

pub use instance_lock::*;
pub use oi_messager::*;
