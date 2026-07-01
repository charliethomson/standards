//! Public short ids — opaque, URL-facing aliases for entities. Lives in `core`.
//!
//! `PublicId<T>` is an 11-character Crockford Base32 code (55 bits) tagged with the
//! entity it names, minted at creation and stored beside the internal `Id<T>`
//! (UUIDv7). It is the *only* identifier that crosses the public boundary — URLs,
//! query params, JSON, event envelopes — and is translated to/from `Id<T>` in the
//! exposer layer (the poem API). The UUID never leaves internal/admin surfaces.
//!
//! This is a handle, not a secret: authorization is still enforced server-side, so
//! knowing a code grants nothing. Runtime representation is 11 ASCII bytes (Copy).
//!
//! Deps: `rand` (minting), `serde` (transport).
use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Crockford Base32 alphabet: digits + A–Z minus the confusable `I L O U`.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 11 symbols × 5 bits = 55 bits of entropy.
const LEN: usize = 11;

pub struct PublicId<T> {
    code: [u8; LEN], // ASCII, canonical uppercase Crockford
    _marker: PhantomData<fn() -> T>,
}

impl<T> PublicId<T> {
    /// Mint a fresh code from 55 random bits. The caller inserts it against a
    /// `UNIQUE` index and regenerates on conflict (a collision is a transparent
    /// retry, not an error).
    #[must_use]
    pub fn new() -> Self {
        Self::from_bits(rand::random::<u64>() & ((1u64 << (5 * LEN)) - 1))
    }

    /// The canonical 11-character string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Safe: `code` only ever holds ASCII bytes drawn from `ALPHABET`.
        std::str::from_utf8(&self.code).expect("code is ASCII")
    }

    fn from_bits(mut bits: u64) -> Self {
        let mut code = [0u8; LEN];
        // Fill least-significant symbol last, so the code reads most-significant first.
        for slot in code.iter_mut().rev() {
            *slot = ALPHABET[(bits & 0x1f) as usize];
            bits >>= 5;
        }
        Self { code, _marker: PhantomData }
    }
}

/// Map one input byte to its 5-bit value, folding Crockford's confusables
/// (`I/L → 1`, `O → 0`) and accepting either case. `U` is not in the alphabet
/// and is rejected.
fn decode_symbol(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'O' | b'o' => Some(0),
        b'I' | b'i' | b'L' | b'l' => Some(1),
        _ => {
            let up = c.to_ascii_uppercase();
            ALPHABET.iter().position(|&a| a == up).map(|p| p as u8)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Length(usize),
    Char(char),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(n) => write!(f, "public id must be {LEN} chars, got {n}"),
            Self::Char(c) => write!(f, "invalid public-id character {c:?}"),
        }
    }
}
impl std::error::Error for ParseError {}

impl<T> FromStr for PublicId<T> {
    type Err = ParseError;
    /// Normalizes (uppercase + confusable folding) and validates, so any spelling
    /// of a code resolves to the same canonical value.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        if bytes.len() != LEN {
            return Err(ParseError::Length(bytes.len()));
        }
        let mut bits: u64 = 0;
        for &c in bytes {
            let v = decode_symbol(c).ok_or(ParseError::Char(c as char))?;
            bits = (bits << 5) | u64::from(v);
        }
        Ok(Self::from_bits(bits))
    }
}

impl<T> Default for PublicId<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Manual impls so the PhantomData type parameter doesn't force `T: Trait` bounds.
impl<T> Clone for PublicId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for PublicId<T> {}
impl<T> PartialEq for PublicId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}
impl<T> Eq for PublicId<T> {}
impl<T> std::hash::Hash for PublicId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.code.hash(state);
    }
}
impl<T> PartialOrd for PublicId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for PublicId<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.code.cmp(&other.code)
    }
}

impl<T> fmt::Display for PublicId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl<T> fmt::Debug for PublicId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicId({})", self.as_str())
    }
}

// Serializes as the bare canonical code string; deserializes through `parse` so
// confusable spellings are normalized on the way in.
impl<T> Serialize for PublicId<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
impl<'de, T> Deserialize<'de> for PublicId<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}
