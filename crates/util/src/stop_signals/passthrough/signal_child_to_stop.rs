//! Contains [signal_child_to_stop] which signals a [Child](std::process::Child)
//! process to stop (the equivalent of sending `SIGINT`).

use std::io;

/// Signals a child process to stop (the equivalent of sending `SIGINT`).
pub fn signal_child_to_stop(child_id: u32) -> Result<(), io::Error> {
    signal_child_to_stop_impl(child_id)
}

#[cfg(windows)]
#[inline(always)]
fn signal_child_to_stop_impl(child_id: u32) -> Result<(), io::Error> {
    use windows_sys::Win32::System::Console;

    if unsafe { Console::GenerateConsoleCtrlEvent(Console::CTRL_C_EVENT, child_id) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
#[inline(always)]
fn signal_child_to_stop_impl(child_id: u32) -> Result<(), io::Error> {
    use libc::{self, pid_t};

    if unsafe { libc::kill(child_id as pid_t, libc::SIGINT) } == -1 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(windows, unix)))]
compile_error!("Unsupported OS.");
