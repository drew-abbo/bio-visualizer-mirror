//! This module exports everything that has to do with MIDI and [streams] of it.

pub mod streams;

use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU8;

use thiserror::Error;

/// A collection of [Key]s with their velocities.
#[derive(Debug, Clone)]
pub struct MidiPacket {
    velocities: [u8; 128],
}

impl MidiPacket {
    /// Create a new [MidiPacket] with all zero velocities.
    pub const fn new() -> Self {
        Self { velocities: [0; _] }
    }

    /// The velocity of the provided [Key].
    pub const fn key_velocity(&self, key: Key) -> u8 {
        self.velocities[key.as_u8() as usize]
    }

    /// Calculates average frequencies of [Key]s that are on in this packet.
    pub fn average_frequency(&self) -> Option<f64> {
        let mut count: usize = 0;
        let mut sum = 0.0;

        for (key, _) in self.key_velocities() {
            sum += key.as_frequency();
            count += 1;
        }

        if count == 0 {
            return None;
        }

        Some(sum / count as f64)
    }

    /// The maximum velocity of all [Key]s in this packet.
    pub fn max_velocity(&self) -> u8 {
        self.velocities.iter().cloned().max().expect("non-zero len")
    }

    /// Whether or not a [Key] has a non-zero velocity in this packet.
    pub fn is_key_on(&self, key: Key) -> bool {
        self.key_velocity(key) != 0
    }

    /// Whether or not a [Key] has zero velocity in this packet.
    pub fn is_key_off(&self, key: Key) -> bool {
        !self.is_key_on(key)
    }

    /// An iterator of all possible keys (see [Key::all_keys]), each with their
    /// velocity from this packet. Also see [Self::on_key_velocities].
    pub fn key_velocities(&self) -> impl Iterator<Item = (Key, u8)> {
        Key::all_keys().map(|key| (key, self.key_velocity(key)))
    }

    /// Like [Self::key_velocities] but pairs with a velocity of 0 are filtered
    /// out.
    pub fn on_key_velocities(&self) -> impl Iterator<Item = (Key, NonZeroU8)> {
        self.key_velocities()
            .filter_map(|(key, vel)| NonZeroU8::new(vel).map(|vel| (key, vel)))
    }

    /// Sets a [Key]'s velocity in the packet.
    pub const fn set_key_velocity(&mut self, key: Key, velocity: u8) {
        self.velocities[key.as_u8() as usize] = velocity;
    }

    /// Sets all [Key]'s velocities to 0.
    pub const fn clear(&mut self) {
        self.velocities = [0; _];
    }
}

impl Default for MidiPacket {
    fn default() -> Self {
        Self::new()
    }
}

/// A musical note that can be represented in MIDI (e.g. *A-1* or *C#*), like a
/// "key" on a piano.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
pub struct Key(u8);

impl Key {
    /// Maximum value for a key.
    pub const MAX_U8: u8 = 127;

    /// Minimum value for a key.
    pub const MIN_U8: u8 = 0;

    /// Create a new [Key] from a [pitch](PitchClass) and an octave. An error is
    /// returned if the key is lower than *C-1* or higher than *G9*.
    pub const fn new(pitch: PitchClass, octave: i8) -> Result<Self, KeyRangeError> {
        let key = (octave as i16 + 1) * 12 + pitch.semitone() as i16;
        match key {
            0..=127 => Ok(Self(key as u8)),
            _ => Err(KeyRangeError),
        }
    }

    /// Create a note from a key value. An error is returned if a value is over
    /// 127.
    pub const fn from_u8(key: u8) -> Result<Self, KeyRangeError> {
        if key <= 127 {
            Ok(Self(key))
        } else {
            Err(KeyRangeError)
        }
    }

    /// The key from a frequency (see [Self::frequency_to_key_float]).
    pub fn from_frequency(frequency: f64) -> Self {
        Self(Self::frequency_to_key_float(frequency).round() as u8)
    }

    /// Maps a frequency value to a key value between 0.0 and 127.0 using equal
    /// temperment.
    pub fn frequency_to_key_float(frequency: f64) -> f64 {
        69.0 + 12.0 * (frequency / 440.0).log2().clamp(0.0, 127.0)
    }

    /// Maps a key value between 0.0 and 127.0 to a frequency using equal
    /// temperment.
    pub fn key_to_frequency(key: f64) -> f64 {
        let key = key.clamp(0.0, 127.0);
        440.0 * 2f64.powf((key - 69.0) / 12.0)
    }

    /// The key's frequency (see [Self::key_to_frequency]).
    pub fn as_frequency(&self) -> f64 {
        Self::key_to_frequency(self.as_u8() as f64)
    }

    /// This note's pitch class.
    pub fn pitch_class(&self) -> PitchClass {
        PitchClass::from_semitone(self.as_u8() % 12).expect("in 0..=12")
    }

    /// This note's octave.
    pub fn octave(&self) -> i8 {
        (self.as_u8() / 12) as i8 - 1
    }

    /// They key's value.
    pub const fn as_u8(&self) -> u8 {
        debug_assert!(self.0 <= Self::MAX_U8);
        self.0
    }

    /// An iterator over all possible keys in order.
    pub fn all_keys() -> impl Iterator<Item = Self> {
        (Self::MIN_U8..(Self::MAX_U8 + 1)).map(|key| Self::from_u8(key).expect("valid key"))
    }

    /// This [Key] formatted as a [str] (e.g. `"A"`, `"C#-1"`, `"B4"`).
    ///
    /// This struct also implements [Display].
    pub const fn as_str(&self) -> &'static str {
        // This function constructs every possible return value at compile time
        // and then returns a reference from the table. This allows this
        // function to be const and skip allocating.

        const BUFS: [(usize, [u8; 4]); 128] = {
            let mut bufs = [(0usize, [b'\0'; 4]); _];

            let mut key = 0;
            while key < 128 {
                let buf = &mut bufs[key as usize];

                const fn push_byte(buf: &mut (usize, [u8; 4]), c: u8) {
                    let (len, buf) = buf;
                    buf[*len] = c;
                    *len += 1;
                }
                const fn push_str(buf: &mut (usize, [u8; 4]), s: &str) {
                    let mut i = 0;
                    while i < s.len() {
                        push_byte(buf, s.as_bytes()[i]);
                        i += 1;
                    }
                }

                // Add pitch string (e.g. "A" or "C#") into the buffer.
                let pitch = PitchClass::from_semitone(key % 12).expect("in 0..=12");
                push_str(buf, pitch.as_str());

                // Add octave string ("-1", "", "1", "2", ...) to the buffer.
                let octave = (key / 12) as i8 - 1;
                match octave {
                    -1 => push_str(buf, "-1"),
                    0 => {}
                    1..=9 => push_byte(buf, b'0' + octave as u8),
                    _ => unreachable!(),
                }

                key += 1
            }

            bufs
        };

        let (len, buf) = &BUFS[self.as_u8() as usize];
        let buf = buf.split_at(*len).0;

        // SAFETY: All possible strings are valid UTF-8.
        unsafe { str::from_utf8_unchecked(buf) }
    }
}

impl From<Key> for u8 {
    fn from(key: Key) -> Self {
        key.as_u8()
    }
}

impl TryFrom<u8> for Key {
    type Error = KeyRangeError;

    fn try_from(key: u8) -> Result<Self, Self::Error> {
        Self::from_u8(key)
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An error converting a [u8] to a [Key].
#[derive(Error, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
#[error("The key value is outside the range of possible MIDI key values.")]
pub struct KeyRangeError;

/// A musical note's pitch class (e.g. *A*, *C#*). Also see [Key].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PitchClass {
    C = 0,
    CSharp = 1,
    D = 2,
    DSharp = 3,
    E = 4,
    F = 5,
    FSharp = 6,
    G = 7,
    GSharp = 8,
    A = 9,
    ASharp = 10,
    B = 11,
}

impl PitchClass {
    /// Create a [PitchClass] from a semitone.
    ///
    /// [None] is returned if `semitone` is not in the range `0..=11`.
    pub const fn from_semitone(semitone: u8) -> Option<Self> {
        Some(match semitone % 12 {
            0 => PitchClass::C,
            1 => PitchClass::CSharp,
            2 => PitchClass::D,
            3 => PitchClass::DSharp,
            4 => PitchClass::E,
            5 => PitchClass::F,
            6 => PitchClass::FSharp,
            7 => PitchClass::G,
            8 => PitchClass::GSharp,
            9 => PitchClass::A,
            10 => PitchClass::ASharp,
            11 => PitchClass::B,
            _ => return None,
        })
    }

    /// Returns the pitch's semitone value in the range `0..=11`.
    pub const fn semitone(self) -> u8 {
        match self {
            PitchClass::C => 0,
            PitchClass::CSharp => 1,
            PitchClass::D => 2,
            PitchClass::DSharp => 3,
            PitchClass::E => 4,
            PitchClass::F => 5,
            PitchClass::FSharp => 6,
            PitchClass::G => 7,
            PitchClass::GSharp => 8,
            PitchClass::A => 9,
            PitchClass::ASharp => 10,
            PitchClass::B => 11,
        }
    }

    /// This [PitchClass] formatted as a [str] (e.g. `"A"`, `"C#"`).
    ///
    /// This struct also implements [Display].
    pub const fn as_str(self) -> &'static str {
        match self {
            PitchClass::C => "C",
            PitchClass::CSharp => "C#",
            PitchClass::D => "D",
            PitchClass::DSharp => "D#",
            PitchClass::E => "E",
            PitchClass::F => "F",
            PitchClass::FSharp => "F#",
            PitchClass::G => "G",
            PitchClass::GSharp => "G#",
            PitchClass::A => "A",
            PitchClass::ASharp => "A#",
            PitchClass::B => "B",
        }
    }
}

impl Display for PitchClass {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
