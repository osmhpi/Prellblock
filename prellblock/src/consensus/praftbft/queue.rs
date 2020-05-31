use std::{collections::VecDeque, ops::Deref, time::Instant};

/// A queue of elements that have an associated insertion time (`inserted`).
///
/// ```
/// # use prellblock::consensus::Queue;
///
/// let mut queue = Queue::default();
///
/// queue.insert(4);
/// queue.insert(1);
/// queue.insert(3);
/// queue.insert(2);
///
/// assert_eq!(queue.remove(&3), Some(3));
/// assert_eq!(queue.remove(&3), None);
///
/// queue.peek().unwrap().inserted().elapsed();
///
/// let data: Vec<_> = queue.collect();
/// assert_eq!(data, [4, 1, 2]);
/// ```
#[derive(Debug)]
pub struct Queue<T> {
    entries: VecDeque<Entry<T>>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}

impl<T> Queue<T> {
    /// Insert an `item` into the queue.
    pub fn insert(&mut self, item: T) {
        self.entries.push_back(Entry::new(item))
    }

    /// Get the number of items in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a reference to the first `Entry` of the queue.
    ///
    /// Use `entry.inserted()` to get the insetion time.
    ///
    /// Entry implements `Deref<Target=T>` to access the `item`.
    #[must_use]
    pub fn peek(&self) -> Option<&Entry<T>> {
        self.entries.front()
    }

    /// Remove an `item` from the queue.
    ///
    /// **Note:** This needs to scan the whole queue
    /// and therefore has an `O(n)` runtime.
    pub fn remove(&mut self, item: &T) -> Option<Entry<T>>
    where
        T: Eq,
    {
        self.entries
            .iter()
            .position(|entry| entry.item == *item)
            .and_then(|index| self.entries.remove(index))
    }

    /// Remove all items in `iter` from the queue.
    ///
    /// **Note:** This needs to scan the whole queue
    /// and therefore has an `O(n * m)` runtime.
    ///
    /// Returns all found entries.
    pub fn remove_all<'a>(&mut self, iter: impl Iterator<Item = &'a T>) -> Vec<Entry<T>>
    where
        T: Eq + 'a,
    {
        iter.filter_map(move |item| self.remove(item)).collect()
    }
}

impl<T> Iterator for Queue<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.entries.pop_front().map(|entry| entry.item)
    }
}

impl<T> Extend<T> for Queue<T> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.entries.extend(iter.into_iter().map(Entry::new));
    }
}

#[derive(Debug)]
pub struct Entry<T> {
    inserted: Instant,
    item: T,
}

impl<T> Entry<T> {
    fn new(item: T) -> Self {
        Self {
            inserted: Instant::now(),
            item,
        }
    }

    pub const fn inserted(&self) -> Instant {
        self.inserted
    }
}

impl<T> Deref for Entry<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}
