//!

pub mod steams;

use std::collections::HashMap;
use thiserror::Error;

/// A collection of [Key]s with their velocities.
#[derive(Debug, Clone, Default)]
pub struct MidiPacket {
    keys: HashMap<Key, u8>,
}

impl MidiPacket {
    /// The velocity of the provided key. If key isn't being tracked it
    /// returns 0.
    pub fn key_velocity(&self, key: Key) -> u8 {
        match self.keys.get(&key) {
            Some(vel) => *vel,
            None => 0,
        }
    }

    /// Calculates average frequencies of keys in this packet.
    pub fn average_frequency(&self) -> Option<f64> {
        let mut count: usize = 0;
        let mut sum = 0.0;

        for (key, _) in self.key_velocities() {
            sum += key.as_frequency() as f64;
            count += 1;
        }

        if count == 0 {
            return None;
        }

        Some(sum / count as f64)
    }

    /// The maximum velocity of keys in this packet.
    pub fn max_velocity(&self) -> u8 {
        self.key_velocities().map(|(_, vel)| vel).max().unwrap_or(0)
    }

    /// Whether or not a key has a non-zero velocity in this packet.
    pub fn is_key_on(&self, key: Key) -> bool {
        self.key_velocity(key) != 0
    }

    /// Whether or not a key has zero velocity in this packet.
    pub fn is_key_off(&self, key: Key) -> bool {
        !self.is_key_on(key)
    }

    /// An iterator of keys and their velocities if the velocities are non-zero.
    pub fn key_velocities(&self) -> impl Iterator<Item = (Key, u8)> {
        self.keys
            .iter()
            .filter(|(_, vel)| **vel != 0)
            .map(|(key, vel)| (*key, *vel))
    }

    /// Sets a key's velocity in the packet.
    pub fn set_key_velocity(&mut self, key: Key, velocity: u8) {
        if velocity == 0 {
            self.keys.remove(&key);
        } else {
            self.keys.insert(key, velocity);
        }
    }

    pub(crate) fn hashmap_capacity(&self) -> usize {
        self.keys.capacity()
    }
}

/// A key on a keyboard.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
pub struct Key(u8);

impl Key {
    /// Maximum value for a key.
    pub const MAX_U8: u8 = 127;

    /// Minimum value for a key.
    pub const MIN_U8: u8 = 0;

    /// Create a note from a key value. An error is returned if a value is over
    /// 127.
    pub const fn from_u8(key: u8) -> Result<Self, KeyFromU8Error> {
        if key <= 127 {
            Ok(Self(key))
        } else {
            Err(KeyFromU8Error)
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

    /// They key's value.
    pub const fn as_u8(&self) -> u8 {
        debug_assert!(self.0 <= Self::MAX_U8);
        self.0
    }
}

impl From<Key> for u8 {
    fn from(key: Key) -> Self {
        key.as_u8()
    }
}

impl TryFrom<u8> for Key {
    type Error = KeyFromU8Error;

    fn try_from(key: u8) -> Result<Self, Self::Error> {
        Self::from_u8(key)
    }
}

/// An error converting a [u8] to a [Key].
#[derive(Error, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
#[error("Could not create key from value over 127")]
pub struct KeyFromU8Error;
