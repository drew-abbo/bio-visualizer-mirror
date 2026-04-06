//! Exports all kinds of [MidiStream]s.

use super::MidiPacket;
use crate::playback_stream::PlaybackStream;

mod live_midi_stream;
pub use live_midi_stream::*;

/// A [PlaybackStream] of [MidiPacket]s.
pub trait MidiStream: PlaybackStream<MidiPacket, MidiStreamError> {}
impl<T: PlaybackStream<MidiPacket, MidiStreamError>> MidiStream for T {}

/// Indicates something went wrong with a [MidiStream] (a [PlaybackStream] of
/// [MidiPacket]s).
#[derive(thiserror::Error, Debug, Clone)]
pub enum MidiStreamError {
    #[error("Failed to read MIDI port")]
    PortError,
    #[error("Failed to connect to a port")]
    ConnectError,
    #[error("Failed to parse MIDI data")]
    DataError,
}
