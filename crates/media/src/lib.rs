//! This library contains all of the functionality for managing media.

pub mod frame;

pub(crate) mod cast_slice;

mod player;
pub use player::VideoPlayer;
