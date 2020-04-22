//! Helper types and functions to handle groups of threads.

use std::{
    fmt::{Debug, Display},
    thread,
    thread::{Builder, JoinHandle},
};

/// A group of threads.
///
/// ```no_run
/// use prellblock::thread_group::ThreadGroup;
///
/// let mut thread_group = ThreadGroup::new();
///
/// thread_group.spawn("Test 1", move || {
///     // This is thread 1
///     // ...
/// });
///
/// thread_group.spawn("Test 2", move || {
///     // This is thread 2
///     // ...
/// });
///
/// thread_group.join_and_log();
/// ```
pub struct ThreadGroup<T> {
    pub(crate) handles: Vec<JoinHandle<T>>,
}

impl<T> ThreadGroup<T> {
    /// Create a new thread group.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    /// Spawn a new thread named `name` in this thread group.
    pub fn spawn<F>(&mut self, name: impl Display, f: F)
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.handles.push(
            Builder::new()
                .name(name.to_string())
                .spawn(f)
                .expect("failed to spawn thread"),
        );
    }

    /// Join all threads in this thread group.
    pub fn join(self) -> impl Iterator<Item = (String, thread::Result<T>)> {
        self.handles.into_iter().map(|handle| {
            (
                handle.thread().name().unwrap_or("<unnamed>").to_string(),
                handle.join(),
            )
        })
    }
}

impl ThreadGroup<()> {
    /// Join all threads in this thread group and log status messages.
    pub fn join_and_log(self) {
        self.join().for_each(log_thread)
    }
}

impl<T, E> ThreadGroup<Result<T, E>>
where
    T: Debug,
    E: Display,
{
    /// Join all threads in this thread group and log status messages.
    pub fn join_and_log(self) {
        self.join().for_each(log_thread_result)
    }
}

pub(crate) fn log_thread((name, result): (String, thread::Result<()>)) {
    match result {
        Err(_) => log::error!("Thread {} panicked.", name),
        Ok(()) => log::info!("Thread {} ended.", name),
    }
}

pub(crate) fn log_thread_result<T, E>((name, result): (String, thread::Result<Result<T, E>>))
where
    T: Debug,
    E: Display,
{
    match result {
        Err(_) => log::error!("Thread {} panicked.", name),
        Ok(Err(err)) => log::error!("Thread {} failed: {}", name, err),
        Ok(Ok(ok)) => log::info!("Thread {} ended: {:?}", name, ok),
    }
}
