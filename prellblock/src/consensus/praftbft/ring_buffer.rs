use std::{convert::TryInto, mem};

/// The `RingBuffer` provides access to a circular buffer with fixed capactiy.
///
/// Do you think "What is a ring buffer?" -> [Wikipedia](https://en.wikipedia.org/wiki/Circular_buffer)
pub(super) struct RingBuffer<T> {
    data: Vec<T>,
    start: u64,
}

impl<T> RingBuffer<T> {
    /// Create a new `RingBuffer` with `capacity` elements.
    /// This will fill the buffer with
    pub(super) fn new(default_item: T, capacity: usize, start: u64) -> Self
    where
        T: Clone,
    {
        Self {
            data: vec![default_item; capacity],
            start,
        }
    }

    /// Return the value stored at the given `index` (relative in the ringbuffer).
    pub(super) fn get(&self, index: u64) -> Option<&T> {
        let index = self.index2index(index)?;
        Some(&self.data[index])
    }

    /// Return a mutable reference
    pub(super) fn get_mut(&mut self, index: u64) -> Option<&mut T> {
        let index = self.index2index(index)?;
        Some(&mut self.data[index])
    }

    pub fn increment(&mut self, new_item: T) -> T {
        // unwrap is ok here, because index2index always returns some with `self.start`
        let old_item = mem::replace(self.get_mut(self.start).unwrap(), new_item);
        self.start += 1;
        old_item
    }

    pub fn increment_to(&mut self, index: impl Into<u64>, new_item: T)
    where
        T: Clone,
    {
        for _i in self.start..index.into() {
            *self.get_mut(self.start).unwrap() = new_item.clone();
            self.start += 1;
        }
    }

    fn index2index(&self, index: u64) -> Option<usize> {
        let len = self.data.len() as u64;
        if index >= self.start && index < self.start + len {
            Some((index % len).try_into().unwrap())
        } else {
            None
        }
    }
}
