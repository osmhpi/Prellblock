use std::{collections::VecDeque, vec::IntoIter};

pub struct FlattenVec<T> {
    iter: IntoIter<T>,
    data: VecDeque<Vec<T>>,
}

impl<T> Default for FlattenVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FlattenVec<T> {
    pub fn new() -> Self {
        Self {
            iter: Vec::new().into_iter(),
            data: VecDeque::new(),
        }
    }

    pub fn push(&mut self, vec: Vec<T>) {
        log::trace!("Push {} transactions.", vec.len());
        self.data.push_back(vec);
    }

    pub fn len(&self) -> usize {
        let iter_len: usize = self.iter.len();
        let data_sum: usize = self.data.iter().map(Vec::len).sum();
        iter_len + data_sum
    }
}

impl<T> Iterator for FlattenVec<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        loop {
            if let Some(item) = self.iter.next() {
                log::trace!("Taking transaction from current iterator.");
                break Some(item);
            } else if let Some(vec) = self.data.pop_front() {
                log::trace!("Using next vector.");
                self.iter = vec.into_iter();
            } else {
                log::trace!("No more transactions.");
                break None;
            }
        }
    }
}
