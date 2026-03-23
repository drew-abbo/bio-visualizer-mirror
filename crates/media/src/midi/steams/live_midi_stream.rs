//! Exports [LiveMidiStream].

use std::sync::Mutex;

use midir::{MidiInput, MidiInputPort};

use super::{MidiPacket, MidiStreamError};
use crate::fps::Fps;
use crate::playback_stream::{PlaybackStream, SeekablePlaybackStream};

#[derive(Debug)]
pub struct LiveMidiStream {
    recycled_packet: Option<MidiPacket>,
    target_fps: Fps,
    paused: bool,
}

impl LiveMidiStream {
    // TODO: Constructor
}

impl PlaybackStream<MidiPacket, MidiStreamError> for LiveMidiStream {
    fn fetch(&mut self) -> Result<MidiPacket, MidiStreamError> {
        todo!() //TODO
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        self.target_fps = new_target_fps;
    }

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn set_paused(&mut self, paused: bool) -> bool {
        self.paused = paused;
        paused
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn seek_controls(
        &mut self,
    ) -> Option<&mut dyn SeekablePlaybackStream<MidiPacket, MidiStreamError>> {
        None
    }

    fn recycle(&mut self, new_packet: MidiPacket) {
        let use_new_packet = match &self.recycled_packet {
            Some(old_packet) if new_packet.hashmap_capacity() > old_packet.hashmap_capacity() => {
                true
            }
            None => true,
            Some(_) => false,
        };

        if use_new_packet {
            self.recycled_packet = Some(new_packet);
        }
    }
}

///
#[derive(Clone)]
pub struct Port<'a> {
    port: MidiInputPort,
    input: &'a MidiInput,
}

impl<'a> Port<'a> {
    /// The name of this port.
    fn port_name(&self) -> Result<String, MidiStreamError> {
        self.input
            .port_name(&self.port)
            .map_err(|_| MidiStreamError::PortError)
    }
}

///TODO: Store a global midi input initialized on first use, it is referenced in the ports we return
pub fn list_ports() -> Result<impl Iterator<Item = Port<'static>>, MidiStreamError> {
    static INPUT: Mutex<Option<MidiInput>> = Mutex::new(None);
    let mut input = INPUT.lock().expect("No Thread Panic.");

    let input: &MidiInput = match &mut *input {
        Some(input) => input,
        None => {
            let new_input =
                MidiInput::new("Midi Client").map_err(|_| MidiStreamError::PortError)?;
            input.insert(new_input)
        }
    };
    Ok(input.ports().into_iter().map(|port| Port { port, input }))
}
