//! This module exports everything that has to do with [Frame]s and
//! [producing](Producer) them.

pub mod streams;

mod buffer;
mod producer;

pub use buffer::*;
pub use producer::*;
