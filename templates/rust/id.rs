//! Typed, sortable entity identifiers. Lives in the `core` crate.
//!
//! `Id<T>` is a UUIDv7 tagged with the entity it identifies, so an `Id<Order>`
//! cannot be passed where an `Id<Item>` is expected. UUIDv7 is time-ordered, so
//! primary-key inserts stay append-friendly and sorting by id approximates
//! creation order. Runtime representation is exactly a `Uuid` (zero-cost tag).
use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub struct Id<T> {
    raw: Uuid,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Mint a fresh, time-ordered identifier.
    #[must_use]
    pub fn new() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    #[must_use]
    pub fn from_uuid(raw: Uuid) -> Self {
        Self { raw, _marker: PhantomData }
    }

    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.raw
    }
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Manual impls so the PhantomData type parameter doesn't force `T: Trait` bounds.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.raw)
    }
}

impl<T> FromStr for Id<T> {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_uuid(Uuid::from_str(s)?))
    }
}

// Serializes as a canonical hyphenated UUID string.
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw.to_string())
    }
}
impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Uuid::from_str(&s).map(Self::from_uuid).map_err(serde::de::Error::custom)
    }
}
