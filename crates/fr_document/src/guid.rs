//! The 128-bit identifier of entities, components and assets.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// Number of hexadecimal characters in the text form of a [`Guid`].
const TEXT_LENGTH: usize = 32;

/// A 128-bit globally unique identifier; all zero means unset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Guid {
    /// The upper 64 bits.
    pub high: u64,
    /// The lower 64 bits.
    pub low: u64,
}

impl Guid {
    /// The unset identifier.
    pub const NONE: Self = Self { high: 0, low: 0 };

    /// Tells whether the identifier is set.
    pub const fn valid(self) -> bool {
        self.high != 0 || self.low != 0
    }

    /// Generates a random, valid identifier.
    pub fn generate() -> Self {
        let value = Self {
            high: random_word(),
            low: random_word(),
        };
        if value.valid() {
            value
        } else {
            Self { high: 0, low: 1 }
        }
    }

    /// Formats the identifier as 32 lowercase hexadecimal digits.
    pub fn to_text(self) -> String {
        format!("{:016x}{:016x}", self.high, self.low)
    }

    /// Parses 32 hexadecimal digits of either case.
    pub fn from_text(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != TEXT_LENGTH {
            return None;
        }
        let mut value = Self::NONE;
        for (index, &byte) in bytes.iter().enumerate() {
            let digit = u64::from(char::from(byte).to_digit(16)?);
            let part = if index < TEXT_LENGTH / 2 {
                &mut value.high
            } else {
                &mut value.low
            };
            *part = (*part << 4) | digit;
        }
        Some(value)
    }
}

/// One random 64-bit word from the operating system's seed, mixed with a counter.
fn random_word() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.finish()
}
