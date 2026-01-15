//! Tools for automatically sending stop signals (e.g. `SIGINT`) to child
//! processes.
//!
//! Enabling this starts up a worker thread, however this thread will remain
//! fully asleep (using 0 CPU) when requests aren't being handled.

// NOTE TO REVIEWER: There is a lot of unsafe and OS-specific FFI code that more
// closely resembles C here (Colin would be proud). This is especially the case
// in the `signal_sleeper` module (and also `signal_child_to_stop`, though to a
// lesser extent). As such, I've relaxed on the safety comments a bit. When
// every function call is unsafe, can you really blame me?

mod receiver_tasks;
mod signal_child_to_stop;
mod signal_sleeper;

use std::hash::{self, Hash};
use std::io;
use std::ops::{Deref, DerefMut};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::{collections::HashSet, sync::MutexGuard};

use signal_hook::{SigId, consts, low_level};

use receiver_tasks::ReceiverTask;

/// Enables the passthrough of stop signals (e.g. `SIGINT`) to child processes.
/// See [register_child] and [disable].
pub fn enable() -> Result<(), io::Error> {
    enable_then(|| {})
}

/// Disables the passthrough of stop signals (e.g. `SIGINT`) to child processes.
/// See [register_child] and [enable].
pub fn disable() {
    let mut passthrough_data = PASSTHROUGH_INIT_DATA
        .lock()
        .expect(super::THREAD_EXPECT_MSG);
    let Some(passthrough_data_inner) = passthrough_data.take() else {
        return;
    };

    let PassthroughData {
        sig_ids,
        receiver_thread,
    } = passthrough_data_inner;

    for sig_id in sig_ids {
        low_level::unregister(sig_id);
    }

    // SAFETY: We've exited the function by this point if the signal sleeper
    // isn't already initialized.
    unsafe { receiver_tasks::send_task(ReceiverTask::Stop) };

    receiver_thread.join().expect(super::THREAD_EXPECT_MSG);

    // SAFETY: We've exited the function by this point if the signal sleeper
    // isn't already initialized.
    unsafe { signal_sleeper::deinit() };
}

/// Returns whether the passthrough of stop signals has been enabled or not (see
/// [enable] and [disable]).
pub fn is_enabled() -> bool {
    PASSTHROUGH_INIT_DATA
        .lock()
        .expect(super::THREAD_EXPECT_MSG)
        .is_some()
}

/// Register a child process to pass stop signals to.
///
/// If not enabled, this function will enable stop signal passthrough (see
/// [enable]). If already enabled, this function will never return an error.
///
/// Also see [register_child_owned] and [unregister_child].
pub fn register_child(child: Arc<Mutex<Child>>) -> Result<(), io::Error> {
    enable_then(|| {
        // SAFETY: The signal sleeper will have been initialized by the time
        // this callback is called.
        unsafe { receiver_tasks::send_task(ReceiverTask::RegisterChild(child)) };
    })
}

/// The same as [register_child] but for when you no longer need the [Child]
/// anymore.
pub fn register_child_owned(child: Child) -> Result<(), io::Error> {
    register_child(Arc::new(Mutex::new(child)))
}

/// Un-register a child process to pass stop signals to.
///
/// If not enabled, this function will enable stop signal passthrough (see
/// [enable]). If already enabled, this function will never return an error.
///
/// Also see [register_child] and [register_child_owned].
pub fn unregister_child(child: Arc<Mutex<Child>>) -> Result<(), io::Error> {
    enable_then(|| {
        // SAFETY: The signal sleeper will have been initialized by the time
        // this callback is called.
        unsafe { receiver_tasks::send_task(ReceiverTask::UnregisterChild(child)) };
    })
}

/// Sends a stop signal to all registered child processes.
///
/// If not enabled, this function will enable stop signal passthrough (see
/// [enable]). If already enabled, this function will never return an error.
pub fn signal_children_to_stop() -> Result<(), io::Error> {
    enable_then(|| {
        // SAFETY: The signal sleeper will have been initialized by the time
        // this callback is called.
        unsafe { receiver_tasks::send_task(ReceiverTask::SignalChildren) };
    })
}

static PASSTHROUGH_INIT_DATA: Mutex<Option<PassthroughData>> = Mutex::new(None);

struct PassthroughData {
    pub sig_ids: [SigId; consts::TERM_SIGNALS.len()],
    pub receiver_thread: JoinHandle<()>,
}

/// A wrapper around a [Child] shared across threads. This type is intended to
/// be used as a key in a hash map/set.
#[derive(Debug)]
struct ChildRef(Arc<Mutex<Child>>);

impl ChildRef {
    // A value that uniquely identifies this process so long as the inner type
    // has at least 1 strong reference.
    pub fn unique_value(&self) -> impl Copy + Eq + Hash {
        Arc::as_ptr(&self.0)
    }
}

impl Deref for ChildRef {
    type Target = Arc<Mutex<Child>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChildRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Arc<Mutex<Child>>> for ChildRef {
    fn from(child: Arc<Mutex<Child>>) -> Self {
        Self(child)
    }
}

impl From<ChildRef> for Arc<Mutex<Child>> {
    fn from(child: ChildRef) -> Self {
        child.0
    }
}

impl PartialEq for ChildRef {
    fn eq(&self, other: &Self) -> bool {
        self.unique_value() == other.unique_value()
    }
}
impl Eq for ChildRef {}

impl Hash for ChildRef {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.unique_value().hash(state);
    }
}

/// A [HashSet] of child processes (see [ChildRef]).
///
/// It's okay to disable `clippy::mutable_key_type` since the contained
/// [ChildRef] type will always use the same value for hashing and equality
/// checks (regardless of changes to the inner [Child]).
#[allow(clippy::mutable_key_type)]
type Children = HashSet<ChildRef>;

/// The same as [enable] but it runs `f` in the critical section after enabling.
#[inline(always)]
fn enable_then<R>(f: impl FnOnce() -> R) -> Result<R, io::Error> {
    let lock_guard = enable_keep_locked()?;
    let ret = f();
    drop(lock_guard);
    Ok(ret)
}

/// The same as [enable] but it returns a locked mutex guard so you can add more
/// work to the critical section with a guarantee that things have been
/// initialized.
///
/// When the [Ok] return value is dropped the critical section is unlocked.
fn enable_keep_locked() -> Result<MutexGuard<'static, Option<PassthroughData>>, io::Error> {
    let mut passthrough_data = PASSTHROUGH_INIT_DATA
        .lock()
        .expect(super::THREAD_EXPECT_MSG);
    if passthrough_data.is_some() {
        return Ok(passthrough_data);
    }

    // SAFETY: We've exited the function by this point if the signal sleeper is
    // already initialized.
    unsafe { signal_sleeper::init() };

    let receiver_thread = thread::spawn(|| {
        #[allow(clippy::mutable_key_type)] // See `Children` documentation.
        let mut children: Children = Children::new();

        loop {
            // SAFETY: The signal sleeper is initialized if this is being called
            // and we're only calling this function from this thread.
            unsafe { signal_sleeper::sleep() };

            match receiver_tasks::receive_task() {
                ReceiverTask::RegisterChild(child) => {
                    children.insert(child.into());
                }
                ReceiverTask::UnregisterChild(child) => {
                    children.remove(&child.into());
                }
                ReceiverTask::SignalChildren => {
                    signal_all_children(&mut children);
                }
                ReceiverTask::Stop => return,
                ReceiverTask::None => unreachable!("We shouldn't be woken without work to do."),
            }
        }
    });

    let sig_ids = super::try_array_from_fn(|i| {
        // SAFETY: The function we're calling is async-signal-safe, does not use
        // mutexes internally, does not allocate memory, and cannot panic, so
        // it's safe to call here.
        unsafe {
            low_level::register(consts::TERM_SIGNALS[i], || {
                // SAFETY: The signal sleeper is initialized if this is being
                // called.
                receiver_tasks::send_task(ReceiverTask::SignalChildren);
            })
        }
    })
    .inspect_err(|e| crate::debug_log_error!("Failed to register signal handler: {e}"))?;

    *passthrough_data = Some(PassthroughData {
        sig_ids,
        receiver_thread,
    });

    Ok(passthrough_data)
}

#[allow(clippy::mutable_key_type)] // See `Children` documentation.
fn signal_all_children(children: &mut Children) {
    children.retain(|child| {
        let mut child = child
            .lock()
            .expect("A thread shouldn't panic with the child.");

        let should_forget_child = match child.try_wait() {
            Ok(None) => false,
            Ok(Some(_)) => true,
            Err(e) => {
                crate::debug_log_error!("Failed to check on child (forgetting child): {e}");
                true
            }
        };

        if !should_forget_child
            && let Err(e) = signal_child_to_stop::signal_child_to_stop(child.id())
        {
            crate::debug_log_error!("Failed to signal child to exit (ignoring): {e}");
        }

        !should_forget_child
    });

    if !children.is_empty() {
        crate::debug_log_info!(
            "Stop signal successfully sent to {} child processes.",
            children.len()
        );
    }
}
