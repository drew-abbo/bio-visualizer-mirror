//! Exports [LiveMidiStream].

use midir::{MidiInput, MidiInputConnection};

use midly::MidiMessage;
use midly::live::LiveEvent;

use util::channels::message_channel::{self, Inbox, Outbox};

use super::{MidiPacket, MidiStreamError};
use crate::fps::Fps;
use crate::midi::Key;
use crate::playback_stream::{PlaybackStream, SeekablePlaybackStream};

/// A [MidiStream](super::MidiStream) that comes from live MIDI input data. See
/// [list_ports].
pub struct LiveMidiStream {
    inbox: Inbox<Msg>,
    recycled_packet: Option<MidiPacket>,
    target_fps: Fps,
    paused: bool,
    _connection: MidiInputConnection<Outbox<Msg>>,
}

impl LiveMidiStream {
    /// Create a new [LiveMidiStream]. See [list_ports].
    pub fn new(port: Port, target_fps: Fps, paused: bool) -> Result<Self, MidiStreamError> {
        let (inbox, outbox) = message_channel::new::<Msg>();

        let input =
            MidiInput::new("New Live Midi Stream").map_err(|_| MidiStreamError::PortError)?;

        let Port {
            id: port_id,
            name: port_name,
        } = port;
        let port = input
            .find_port_by_id(port_id)
            .ok_or(MidiStreamError::PortError)?;

        let callback = |_timestamp: u64, message_bytes: &[u8], outbox: &mut Outbox<Msg>| {
            let event = match LiveEvent::parse(message_bytes) {
                Ok(event) => event,
                Err(_) => {
                    _ = outbox.send(Err(MidiStreamError::DataError));
                    return;
                }
            };

            let LiveEvent::Midi { message, .. } = event else {
                return;
            };

            let (key, note_state) = match message {
                MidiMessage::NoteOff { key, .. } => (key, MidiNoteState::Off),
                MidiMessage::NoteOn { key, vel } => (key, MidiNoteState::On { vel: vel.as_int() }),
                _ => return,
            };
            let key = key.as_int().try_into().expect("u7 can't be over 127");

            _ = outbox.send(Ok((key, note_state)));
        };

        let connection = input
            .connect(&port, &port_name, callback, outbox)
            .map_err(|_| MidiStreamError::ConnectError)?;

        Ok(Self {
            inbox,
            recycled_packet: None,
            target_fps,
            paused,
            _connection: connection,
        })
    }
}

impl PlaybackStream<MidiPacket, MidiStreamError> for LiveMidiStream {
    fn fetch(&mut self) -> Result<MidiPacket, MidiStreamError> {
        let mut packet = self
            .recycled_packet
            .take()
            .map(|mut packet| {
                packet.hashmap_mut().clear();
                packet
            })
            .unwrap_or_default();

        let check_result = self
            .inbox
            .check_in_place(|msg_queue| {
                for msg in msg_queue.drain(..) {
                    let (key, note_state) = match msg {
                        Ok(msg) => msg,
                        Err(e) => return Err(e),
                    };

                    if self.paused {
                        continue;
                    }

                    match note_state {
                        MidiNoteState::On { vel } => packet.hashmap_mut().insert(key, vel),
                        MidiNoteState::Off => packet.hashmap_mut().remove(&key),
                    };
                }

                Ok(())
            })
            .expect("Thread didn't panic");

        if let Some(Err(e)) = check_result {
            return Err(e);
        }

        Ok(packet)
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
            Some(old_packet)
                if new_packet.hashmap().capacity() > old_packet.hashmap().capacity() =>
            {
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

/// A MIDI input port. See [list_ports].
pub struct Port {
    id: String,
    name: String,
}

impl Port {
    /// The name of this port.
    pub const fn port_name(&self) -> &str {
        self.name.as_str()
    }
}

/// A list of every MIDI input [Port] that can be used to construct a
/// [LiveMidiStream].
pub fn list_ports() -> Result<impl Iterator<Item = Port>, MidiStreamError> {
    let input = MidiInput::new("Midi Input").map_err(|_| MidiStreamError::PortError)?;
    let ports = input.ports();

    let mut ret = Vec::with_capacity(ports.len());

    for port in ports {
        ret.push(Port {
            id: port.id(),
            name: input
                .port_name(&port)
                .map_err(|_| MidiStreamError::PortError)?,
        })
    }
    Ok(ret.into_iter())
}

type Msg = Result<(Key, MidiNoteState), MidiStreamError>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MidiNoteState {
    On { vel: u8 },
    Off,
}
