//! This module contains the [DropJoinHandle] type, a thin wrapper type around
//! [JoinHandle] that joins the thread when the handle is dropped (RAII style).

use std::ops::{Deref, DerefMut};
use std::thread::{self, JoinHandle};

/// A thin wrapper around [JoinHandle] that joins the thread when the handle is
/// dropped (RAII style).
///
/// Any error in joining the thread will be ignored.
#[derive(Debug)]
pub struct DropJoinHandle<T>(Option<JoinHandle<T>>);

impl<T> DropJoinHandle<T> {
    /// Create from an existing join handle.
    pub fn new(handle: JoinHandle<T>) -> Self {
        Self::from(handle)
    }
}

impl<T> Deref for DropJoinHandle<T> {
    type Target = JoinHandle<T>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect(EXPECT_MSG)
    }
}

impl<T> DerefMut for DropJoinHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect(EXPECT_MSG)
    }
}

impl<T> From<JoinHandle<T>> for DropJoinHandle<T> {
    fn from(handle: JoinHandle<T>) -> Self {
        DropJoinHandle(Some(handle))
    }
}

impl<T> From<DropJoinHandle<T>> for JoinHandle<T> {
    fn from(mut handle: DropJoinHandle<T>) -> Self {
        handle.0.take().expect(EXPECT_MSG)
    }
}

impl<T> Drop for DropJoinHandle<T> {
    fn drop(&mut self) {
        _ = self.0.take().expect(EXPECT_MSG).join();
    }
}

/// The same as [thread::spawn], but a [DropJoinHandle] is returned instead.
pub fn spawn<F, T>(f: F) -> DropJoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    DropJoinHandle::from(thread::spawn(f))
}

const EXPECT_MSG: &str = "The handle should be present.";
