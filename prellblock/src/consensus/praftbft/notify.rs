#![allow(clippy::module_name_repetitions)]

use std::{
    collections::HashMap,
    future::Future,
    hash::Hash,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

type NotifyMapInner<T> = Arc<Mutex<HashMap<T, slab::Slab<Option<Waker>>>>>;

/// A structure that can notify multiple tasks once a given state is reached.
#[derive(Debug)]
pub struct NotifyMap<T>
where
    T: Eq + Hash,
{
    inner: NotifyMapInner<T>,
}

impl<T> Default for NotifyMap<T>
where
    T: Eq + Hash,
{
    fn default() -> Self {
        Self {
            inner: Arc::default(),
        }
    }
}

impl<T> NotifyMap<T>
where
    T: Eq + Hash,
{
    /// Wait until a given `state` is reached.
    ///
    /// The returned `Wait` future will resolve once
    /// `notify_all(state)` is called or the `NotifyMap` is dropped.
    pub fn wait(&mut self, state: T) -> Wait<T> {
        Wait {
            index: 0,
            inner: self.inner.clone(),
            state,
        }
    }

    /// Notify all futures waiting for a given `state`.
    pub fn notify_all(&mut self, state: &T) {
        if let Some(wakers) = self.inner.lock().unwrap().get_mut(state) {
            for (_, waker) in wakers {
                if let Some(waker) = waker.take() {
                    waker.wake();
                }
            }
        }
    }
}

impl<T> Drop for NotifyMap<T>
where
    T: Hash + Eq,
{
    fn drop(&mut self) {
        // Notify all futurues waiting for any state.
        for wakers in self.inner.lock().unwrap().values_mut() {
            for (_, waker) in wakers {
                if let Some(waker) = waker.take() {
                    waker.wake();
                }
            }
        }
    }
}

/// A future that waits until it is notified or the notifier is dropped.
pub struct Wait<T>
where
    T: Hash + Eq,
{
    // `0` if no waker is registered
    // `1 + key` otherwise
    index: usize,
    inner: NotifyMapInner<T>,
    state: T,
}

impl<T> Clone for Wait<T>
where
    T: Hash + Eq + Clone,
{
    fn clone(&self) -> Self {
        Self {
            index: 0,
            inner: self.inner.clone(),
            state: self.state.clone(),
        }
    }
}

impl<T> Future for Wait<T>
where
    T: Hash + Eq + Clone + Unpin,
{
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();
        match (self.index.checked_sub(1), inner.get_mut(&self.state)) {
            // A waker is already registered.
            (Some(index), Some(wakers)) => {
                match &mut wakers[index] {
                    // Waker was notified.
                    None => return Poll::Ready(()),
                    // A waker is already configured, no need to clone the waker.
                    Some(waker) if waker.will_wake(ctx.waker()) => {}
                    // Setup a new waker, the old one is no longer valid.
                    Some(waker) => *waker = ctx.waker().clone(),
                }
            }
            // A waker is already registered, but the waker list is removed.
            (Some(_), None) => unreachable!(),
            // No waker is registered and the wakers list already exists.
            (None, Some(wakers)) => {
                let index = wakers.insert(Some(ctx.waker().clone()));
                drop(inner);
                self.index = 1 + index;
            }
            // No waker is registered and there is no wakers list.
            (None, None) => {
                let mut wakers = slab::Slab::new();
                let index = wakers.insert(Some(ctx.waker().clone()));
                inner.insert(self.state.clone(), wakers);
                drop(inner);
                self.index = 1 + index;
            }
        }
        Poll::Pending
    }
}

impl<T> Drop for Wait<T>
where
    T: Hash + Eq,
{
    fn drop(&mut self) {
        // Remove our waker from the waker list.
        if let Some(index) = self.index.checked_sub(1) {
            let mut inner = self.inner.lock().unwrap();
            let wakers = inner.get_mut(&self.state).unwrap();
            wakers.remove(index);

            // Remove the wakers list if it's empty.
            if wakers.is_empty() {
                inner.remove(&self.state);
            }
        }
    }
}
