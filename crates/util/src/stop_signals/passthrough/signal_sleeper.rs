//! A platform specific implementation for signaling a worker thread to wake up
//! and sleep where the wake operation uses no mutexes, no memory allocations,
//! and no other functions that aren't async-signal-safe (so it can be called
//! from within a signal handler).

/// Initializes the signal sleeper.
///
/// # Safety
///
/// You cannot call this twice without calling [deinit] first.
pub unsafe fn init() {
    unsafe { SignalSleeperImpl::init() }
}

/// De-initializes the signal sleeper.
///
/// # Safety
///
/// [init] must be called once before this is called (every time). This *cannot*
/// be called while a thread is inside [sleep].
pub unsafe fn deinit() {
    unsafe { SignalSleeperImpl::deinit() }
}

/// Consumes a notification or blocks until one can be consumed. Intended to be
/// called by a thread to sleep until [wake] is called by another thread.
///
/// # Safety
///
/// This can only be called in-between [init] and [deinit] calls by one thread
/// at a time.
pub unsafe fn sleep() {
    unsafe { SignalSleeperImpl::sleep() }
}

/// Records a notification. Intended to be called by a non-worker thread to get
/// a worker thread to do work.
///
/// This function is async-signal-safe, does not use mutexes internally, does
/// not allocate memory, and cannot panic.
///
/// # Safety
///
/// This can only be called in-between [init] and [deinit] calls by one thread
/// at a time.
pub unsafe fn wake() {
    unsafe { SignalSleeperImpl::wake() }
}

trait SignalSleeperTrait {
    unsafe fn init();
    unsafe fn deinit();
    unsafe fn sleep();
    unsafe fn wake();
}

#[derive(Debug)]
struct SignalSleeperImpl;

#[cfg(windows)]
mod signal_sleeper_impl {
    use super::{SignalSleeperImpl, SignalSleeperTrait};

    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicPtr, Ordering};

    use windows_sys::Win32::{Foundation, System::Threading};

    use signal_hook::low_level;

    static EVENT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

    impl SignalSleeperTrait for SignalSleeperImpl {
        unsafe fn init() {
            let handle = unsafe {
                Threading::CreateEventW(
                    ptr::null_mut(),
                    1, // manual reset
                    0, // initially unsignaled
                    ptr::null(),
                )
            };
            assert!(!handle.is_null(), "Failed to create event handle.");

            EVENT.store(handle, Ordering::Release);
        }

        unsafe fn deinit() {
            let handle = EVENT.swap(ptr::null_mut(), Ordering::AcqRel);
            assert_ne!(
                unsafe { Foundation::CloseHandle(handle) },
                0,
                "Failed to close event handle."
            )
        }

        unsafe fn sleep() {
            let handle = EVENT.load(Ordering::Acquire);

            loop {
                let wait_result = unsafe { Threading::WaitForSingleObject(handle, u32::MAX) };
                if wait_result == Foundation::WAIT_TIMEOUT {
                    continue;
                }

                assert_eq!(
                    wait_result,
                    Foundation::WAIT_OBJECT_0,
                    "Failed to wait for event."
                );
                break;
            }

            assert_ne!(
                unsafe { Threading::ResetEvent(handle) },
                0,
                "Failed to reset event."
            );
        }

        unsafe fn wake() {
            let handle = EVENT.load(Ordering::Relaxed);

            if unsafe { Threading::SetEvent(handle) } == 0 {
                // Panicking here is UB. Instead we'll just abort.
                low_level::abort();
            }
        }
    }
}

#[cfg(unix)]
mod signal_sleeper_impl {
    use super::{SignalSleeperImpl, SignalSleeperTrait};

    use std::ffi::{c_int, c_void};
    use std::io;
    use std::sync::atomic::{AtomicI32, Ordering};

    use signal_hook::low_level;

    const _: () = assert!(
        size_of::<c_int>() == size_of::<i32>(),
        "File descriptors can't be stored as i32 on this platform."
    );

    static READ_FD: AtomicI32 = AtomicI32::new(-1);
    static WRITE_FD: AtomicI32 = AtomicI32::new(-1);

    fn get_errno() -> c_int {
        // NOTE: `libc::__errno_location()` is not available on all platforms,
        // so we have to do this:
        io::Error::last_os_error().raw_os_error().unwrap_or(0) as c_int
    }

    macro_rules! libc_try {
        ($expr:expr) => {{
            let ret = $expr;
            if ret >= 0 { Ok(ret) } else { Err(ret) }
        }};
    }

    impl SignalSleeperTrait for SignalSleeperImpl {
        unsafe fn init() {
            let mut fds = [0i32; 2];
            libc_try!(unsafe { libc::pipe(fds.as_mut_ptr()) })
                .expect("Opening pipe shouldn't fail.");

            let read_fd = fds[0];
            let write_fd = fds[1];
            debug_assert!(
                read_fd >= 0 && write_fd >= 0,
                "Sanity check: No negative file descriptors."
            );

            // Non-blocking to avoid deadlock.
            libc_try!(unsafe { libc::fcntl(read_fd, libc::F_SETFL, libc::O_NONBLOCK) })
                .and_then(|_| {
                    libc_try!(unsafe { libc::fcntl(write_fd, libc::F_SETFL, libc::O_NONBLOCK) })
                })
                .expect("Making pipe non-blocking shouldn't fail.");

            READ_FD.store(read_fd, Ordering::Release);
            WRITE_FD.store(write_fd, Ordering::Release);
        }

        unsafe fn deinit() {
            let read_fd = READ_FD.swap(-1, Ordering::AcqRel);
            let write_fd = WRITE_FD.swap(-1, Ordering::AcqRel);

            libc_try!(unsafe { libc::close(read_fd) })
                .and_then(|_| libc_try!(unsafe { libc::close(write_fd) }))
                .expect("Closing pipe shouldn't fail.");
        }

        unsafe fn sleep() {
            let read_fd = READ_FD.load(Ordering::Acquire);

            loop {
                let mut buf = [0u8];
                let bytes_read = libc_try!(unsafe {
                    libc::read(read_fd, buf.as_mut_ptr() as *mut c_void, buf.len())
                });

                if let Ok(bytes_read) = bytes_read {
                    if bytes_read == 0 {
                        continue;
                    }
                    break;
                }

                assert_eq!(get_errno(), libc::EINTR, "Pipe read failed unexpectedly.");
            }
        }

        unsafe fn wake() {
            let write_fd = WRITE_FD.load(Ordering::Relaxed);

            loop {
                let buf = [1u8];
                let bytes_written = libc_try!(unsafe {
                    libc::write(write_fd, buf.as_ptr() as *const c_void, buf.len())
                });

                if let Ok(bytes_written) = bytes_written {
                    if bytes_written == 0 {
                        continue;
                    }
                    break;
                }

                let err = get_errno();
                if err != libc::EAGAIN && err != libc::EINTR {
                    // Panicking here is UB. Instead we'll just abort.
                    low_level::abort();
                }
            }
        }
    }
}

#[cfg(not(any(windows, unix)))]
compile_error!("Unsupported OS.");
