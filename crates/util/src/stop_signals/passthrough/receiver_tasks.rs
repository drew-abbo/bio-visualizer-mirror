//! Tools for sending and receiving tasks to a worker thread that is handling
//! signal forwarding.

use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{hint, ptr};

use super::signal_sleeper;

/// A task for the receiver thread to wake up and do.
#[repr(usize)]
#[derive(Debug, Clone)]
pub enum ReceiverTask {
    RegisterChild(Arc<Mutex<Child>>) = ReceiverTaskShallow::REGISTER_CHILD_VALUE,
    UnregisterChild(Arc<Mutex<Child>>) = ReceiverTaskShallow::UNREGISTER_CHILD_VALUE,
    SignalChildren = ReceiverTaskShallow::SIGNAL_CHILDREN_VALUE,
    Stop = ReceiverTaskShallow::STOP_VALUE,
    None = ReceiverTaskShallow::NONE_VALUE,
}

impl ReceiverTask {
    const fn from_shallow(task: ReceiverTaskShallow) -> Self {
        match task {
            ReceiverTaskShallow::RegisterChild | ReceiverTaskShallow::UnregisterChild => {
                panic!("Missing child argument")
            }
            ReceiverTaskShallow::SignalChildren => Self::SignalChildren,
            ReceiverTaskShallow::Stop => Self::Stop,
            ReceiverTaskShallow::None => Self::None,
        }
    }

    fn from_shallow_with_child(task: ReceiverTaskShallow, child: Arc<Mutex<Child>>) -> Self {
        match task {
            ReceiverTaskShallow::RegisterChild => Self::RegisterChild(child),
            ReceiverTaskShallow::UnregisterChild => Self::UnregisterChild(child),
            _ => Self::from_shallow(task),
        }
    }

    const fn as_shallow(&self) -> ReceiverTaskShallow {
        match *self {
            Self::RegisterChild(_) => ReceiverTaskShallow::RegisterChild,
            Self::UnregisterChild(_) => ReceiverTaskShallow::UnregisterChild,
            Self::SignalChildren => ReceiverTaskShallow::SignalChildren,
            Self::Stop => ReceiverTaskShallow::Stop,
            Self::None => ReceiverTaskShallow::None,
        }
    }

    const fn as_num(&self) -> usize {
        self.as_shallow().as_num()
    }

    fn into_child(self) -> Option<Arc<Mutex<Child>>> {
        match self {
            Self::RegisterChild(child) => Some(child),
            Self::UnregisterChild(child) => Some(child),
            Self::SignalChildren => None,
            Self::Stop => None,
            Self::None => None,
        }
    }
}

/// Sends a receiver task, blocking until it is received. This function will
/// execute sequencially (when called from mutliple threads one message gets
/// sent and then received at a time).
///
/// This function is async-signal-safe, does not use mutexes internally, does
/// not allocate memory, and cannot panic.
///
/// # Safety
///
/// [signal_sleeper] must be initialized.
pub unsafe fn send_task(task: ReceiverTask) {
    acquire_task_lock();

    #[cfg(debug_assertions)]
    {
        use signal_hook::low_level;

        let curr_task_is_none =
            ReceiverTaskShallow::from_num(TASK.load(Ordering::Acquire)).is_none();
        let curr_child_is_null = TASK_REGISTRATION_CHILD.load(Ordering::Acquire).is_null();
        if !curr_task_is_none || !curr_child_is_null {
            // We can't log or panic here because of async-signal-safety (this
            // may make things hard to debug but I'd rather that than UB).
            low_level::abort();
        }
    }

    let shallow_task = task.as_shallow();
    if let Some(child) = task.into_child() {
        // We cast to a mut pointer here so we can store in the `AtomicPtr`. The
        // data we're pointing to is still const.
        TASK_REGISTRATION_CHILD.store(Arc::into_raw(child) as *mut Mutex<Child>, Ordering::SeqCst);
    }
    TASK.store(shallow_task.as_num(), Ordering::SeqCst);

    // SAFETY: On the caller to ensure the signal sleeper has been initialized.
    unsafe { signal_sleeper::wake() };

    // Wait for our data to be loaded.
    while !ReceiverTaskShallow::from_num(TASK.load(Ordering::SeqCst)).is_none() {
        hint::spin_loop();
    }
    if shallow_task.needs_child() {
        while !TASK_REGISTRATION_CHILD.load(Ordering::SeqCst).is_null() {
            hint::spin_loop();
        }
    }

    release_task_lock();
}

/// Receives a receiver task, consuming it and replacing it with
/// [ReceiverTask::None] (the return value may still be [ReceiverTask::None] if
/// nothing has been sent).
pub fn receive_task() -> ReceiverTask {
    let task: ReceiverTaskShallow = TASK
        .swap(ReceiverTaskShallow::None.as_num(), Ordering::AcqRel)
        .into();

    if task.needs_child() {
        let child_ptr =
            TASK_REGISTRATION_CHILD.swap(ptr::null_mut(), Ordering::AcqRel) as *const Mutex<Child>;
        debug_assert!(!child_ptr.is_null());

        // SAFETY: This pointer was created with `Arc::into_raw`.
        let child = unsafe { Arc::from_raw(child_ptr) };

        return ReceiverTask::from_shallow_with_child(task, child);
    }

    ReceiverTask::from_shallow(task)
}

static TASK: AtomicUsize = AtomicUsize::new(ReceiverTask::None.as_num());
static TASK_REGISTRATION_CHILD: AtomicPtr<Mutex<Child>> = AtomicPtr::new(ptr::null_mut());
static TASK_LOCK: AtomicBool = AtomicBool::new(false);

/// The same as [ReceiverTask], just without actually storing any data.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiverTaskShallow {
    RegisterChild = Self::REGISTER_CHILD_VALUE,
    UnregisterChild = Self::UNREGISTER_CHILD_VALUE,
    SignalChildren = Self::SIGNAL_CHILDREN_VALUE,
    Stop = Self::STOP_VALUE,
    None = Self::NONE_VALUE,
}

impl ReceiverTaskShallow {
    pub const REGISTER_CHILD_VALUE: usize = 1;
    pub const UNREGISTER_CHILD_VALUE: usize = 2;
    pub const SIGNAL_CHILDREN_VALUE: usize = 3;
    pub const STOP_VALUE: usize = 4;
    pub const NONE_VALUE: usize = 0;

    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub const fn from_num(num: usize) -> Self {
        match num {
            Self::REGISTER_CHILD_VALUE => Self::RegisterChild,
            Self::UNREGISTER_CHILD_VALUE => Self::UnregisterChild,
            Self::SIGNAL_CHILDREN_VALUE => Self::SignalChildren,
            Self::STOP_VALUE => Self::Stop,
            _ => Self::None,
        }
    }

    pub const fn as_num(&self) -> usize {
        match self {
            Self::RegisterChild => Self::REGISTER_CHILD_VALUE,
            Self::UnregisterChild => Self::UNREGISTER_CHILD_VALUE,
            Self::SignalChildren => Self::SIGNAL_CHILDREN_VALUE,
            Self::Stop => Self::STOP_VALUE,
            Self::None => Self::NONE_VALUE,
        }
    }

    const fn needs_child(&self) -> bool {
        matches!(self, Self::RegisterChild | Self::UnregisterChild)
    }
}

impl From<usize> for ReceiverTaskShallow {
    fn from(num: usize) -> Self {
        Self::from_num(num)
    }
}

impl From<ReceiverTaskShallow> for usize {
    fn from(task: ReceiverTaskShallow) -> Self {
        task.as_num()
    }
}

fn acquire_task_lock() {
    while TASK_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        hint::spin_loop();
    }
}

fn release_task_lock() {
    TASK_LOCK.store(false, Ordering::Release);
}
