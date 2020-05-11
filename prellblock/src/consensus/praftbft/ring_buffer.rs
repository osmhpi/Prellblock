use std::{
    cmp::Ord,
    convert::TryInto,
    fmt::Debug,
    mem,
    ops::{AddAssign, SubAssign},
};

#[derive(Debug)]
pub enum Error<I> {
    RingbufferUnderflow(I),
    RingbufferOverflow(I),
}

/// The `RingBuffer` provides access to a circular buffer with fixed capactiy.
///
/// Do you think "What is a ring buffer?" -> [Wikipedia](https://en.wikipedia.org/wiki/Circular_buffer)
#[derive(Debug)]
pub struct RingBuffer<I, T> {
    data: Vec<T>,
    start: I,
}

impl<I, T> RingBuffer<I, T>
where
    I: Into<u64> + AddAssign<u64> + Ord + Copy + Debug,
{
    /// Create a new `RingBuffer` with `capacity` elements cloned from
    /// `default_item` starting at `start`.
    pub fn new(default_item: T, capacity: usize, start: I) -> Self
    where
        T: Clone,
    {
        Self {
            data: vec![default_item; capacity],
            start,
        }
    }

    /// Get the current start index.
    pub fn start(&self) -> I {
        self.start
    }

    /// Get the current end index.
    pub fn end(&self) -> I {
        let mut end = self.start;
        end += self.data.len() as u64;
        end
    }

    fn first_mut(&mut self) -> &mut T {
        let index = self.index2index_unchecked(self.start);
        &mut self.data[index]
    }

    /// Return a reference to the item at `index`.
    pub fn get(&self, index: I) -> Result<&T, Error<I>> {
        let index = self.index2index(index)?;
        Ok(&self.data[index])
    }

    /// Return a mutable reference to the item at `index`.
    pub fn get_mut(&mut self, index: I) -> Result<&mut T, Error<I>> {
        let index = self.index2index(index)?;
        Ok(&mut self.data[index])
    }

    /// Increment the start index.
    pub fn increment(&mut self, new_item: T) -> T {
        // unwrap is ok here, because index2index always returns some with `self.start`
        let old_item = mem::replace(self.first_mut(), new_item);
        self.start += 1;
        old_item
    }

    /// Increment the start index to a new `index`.
    ///
    /// This does nothing if `self.start() >= index`.
    pub fn increment_to(&mut self, index: I, new_item: T)
    where
        T: Clone,
    {
        for _ in self.start.into()..index.into() {
            self.increment(new_item.clone());
        }
    }

    /// Decrement the start index.
    pub fn decrement(&mut self, new_item: T) -> T
    where
        I: SubAssign<u64>,
    {
        self.start -= 1;
        mem::replace(self.first_mut(), new_item)
    }

    fn index2index(&self, index: I) -> Result<usize, Error<I>> {
        if index < self.start() {
            Err(Error::RingbufferUnderflow(index))
        } else if index < self.end() {
            Ok(self.index2index_unchecked(index))
        } else {
            Err(Error::RingbufferOverflow(index))
        }
    }

    fn index2index_unchecked(&self, index: I) -> usize {
        let len = self.data.len() as u64;
        (index.into() % len).try_into().unwrap()
    }
}
