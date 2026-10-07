//! Generational handles and the slot map that issues them.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A reference to an object in a [`SlotMap`] that goes stale when the object is removed.
///
/// `Tag` only distinguishes handle kinds; it adds no bounds and no size. Handles order by
/// index, then generation. A generation of zero is never issued.
pub struct Handle<Tag> {
    /// The slot the object lives in.
    index: u32,
    /// How many objects have lived in the slot before this one, plus one.
    generation: u32,
    /// Marks the kind of object the handle names.
    tag: PhantomData<fn() -> Tag>,
}

impl<Tag> Handle<Tag> {
    /// The handle that names nothing.
    pub const NULL: Self = Self {
        index: 0,
        generation: 0,
        tag: PhantomData,
    };

    /// The slot index of the object, which is stable while the object lives.
    pub const fn index(self) -> usize {
        self.index as usize
    }

    /// Whether the handle was ever issued; a stale handle is still non-null.
    pub const fn is_null(self) -> bool {
        self.generation == 0
    }
}

impl<Tag> Clone for Handle<Tag> {
    /// Copies the handle.
    fn clone(&self) -> Self {
        *self
    }
}

impl<Tag> Copy for Handle<Tag> {}

impl<Tag> Default for Handle<Tag> {
    /// The null handle.
    fn default() -> Self {
        Self::NULL
    }
}

impl<Tag> PartialEq for Handle<Tag> {
    /// Compares index and generation.
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<Tag> Eq for Handle<Tag> {}

impl<Tag> Hash for Handle<Tag> {
    /// Hashes index and generation.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<Tag> PartialOrd for Handle<Tag> {
    /// Orders by index, then generation.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tag> Ord for Handle<Tag> {
    /// Orders by index, then generation.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index
            .cmp(&other.index)
            .then(self.generation.cmp(&other.generation))
    }
}

impl<Tag> fmt::Debug for Handle<Tag> {
    /// Writes the index and generation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}, {})", self.index, self.generation)
    }
}

/// One slot of a [`SlotMap`].
struct Slot<T> {
    /// The object while the slot is occupied.
    value: Option<T>,
    /// The generation the next handle of this slot gets.
    generation: u32,
}

/// Objects addressed by [`Handle`]. Vacant slots are reused last in, first out, and
/// iteration visits occupied slots in index order, so it never depends on hashing.
pub struct SlotMap<T, Tag> {
    /// Every slot ever made.
    slots: Vec<Slot<T>>,
    /// The indices of vacant slots.
    free: Vec<u32>,
    /// The number of occupied slots.
    len: usize,
    /// Marks the kind of handle the map issues.
    tag: PhantomData<fn() -> Tag>,
}

impl<T, Tag> Default for SlotMap<T, Tag> {
    /// An empty map.
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
            tag: PhantomData,
        }
    }
}

impl<T, Tag> SlotMap<T, Tag> {
    /// Stores `value` and returns its handle.
    pub fn insert(&mut self, value: T) -> Handle<Tag> {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.value = Some(value);
            return Handle {
                index,
                generation: slot.generation,
                tag: PhantomData,
            };
        }
        let index = self.slots.len() as u32;
        self.slots.push(Slot {
            value: Some(value),
            generation: 1,
        });
        Handle {
            index,
            generation: 1,
            tag: PhantomData,
        }
    }

    /// Removes the object `handle` names and returns it.
    pub fn remove(&mut self, handle: Handle<Tag>) -> Option<T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        self.free.push(handle.index);
        self.len -= 1;
        Some(value)
    }

    /// The object `handle` names, if it is live.
    pub fn get(&self, handle: Handle<Tag>) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    /// The object `handle` names, mutably, if it is live.
    pub fn get_mut(&mut self, handle: Handle<Tag>) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Whether `handle` names a live object.
    pub fn contains(&self, handle: Handle<Tag>) -> bool {
        self.get(handle).is_some()
    }

    /// The number of live objects.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no object is live.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of slots ever made, which bounds every live handle index.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// The handle of the live object in slot `index`, if there is one.
    pub fn handle_at(&self, index: usize) -> Option<Handle<Tag>> {
        let slot = self.slots.get(index)?;
        slot.value.as_ref()?;
        Some(Handle {
            index: index as u32,
            generation: slot.generation,
            tag: PhantomData,
        })
    }

    /// The live objects with their handles in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<Tag>, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    Handle {
                        index: index as u32,
                        generation: slot.generation,
                        tag: PhantomData,
                    },
                    value,
                )
            })
        })
    }

    /// The live handles in slot order.
    pub fn handles(&self) -> impl Iterator<Item = Handle<Tag>> + '_ {
        self.iter().map(|(handle, _)| handle)
    }
}
