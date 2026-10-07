//! Typed index handles of the forge engine.
//!
//! A [`Handle<T>`] names one entry of a table of `T` by its index. The type
//! parameter only keeps handles of different kinds apart; each data crate
//! names its own, such as `MeshId` in `fr_mesh`.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// The index of one entry of a table of `T`.
pub struct Handle<T> {
    /// The position in the table.
    index: u32,
    /// Ties the handle to the kind of entry it names.
    kind: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Wraps `index`; only the owner of the table should call this.
    pub const fn from_index(index: u32) -> Self {
        Self {
            index,
            kind: PhantomData,
        }
    }

    /// The index this handle was created from.
    pub const fn index(self) -> usize {
        self.index as usize
    }
}

impl<T> Clone for Handle<T> {
    /// Copies the handle.
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> fmt::Debug for Handle<T> {
    /// Writes the index.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Handle").field(&self.index).finish()
    }
}

impl<T> PartialEq for Handle<T> {
    /// Handles are equal when their indices are.
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    /// Hashes the index.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> PartialOrd for Handle<T> {
    /// Orders by index.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Handle<T> {
    /// Orders by index.
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}
