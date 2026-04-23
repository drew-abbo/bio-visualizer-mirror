//! Exports the `debug_assertions` implementation for [super::init].

use std::backtrace::Backtrace;
use std::fs::File;
use std::io::{self, Write};
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use time::{self, OffsetDateTime};

use crate::local_data;

/// Implementation for [super::init].
pub fn init_impl() {
    // Don't allow multiple initializations.
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        // Ensure only 1 thread panics at a time:
        static GUARD: AtomicBool = AtomicBool::new(false);
        if GUARD.swap(true, Ordering::SeqCst) {
            // We wait in case another thread is reporting a crash.
            thread::sleep(Duration::from_secs(5));
            process::abort();
        }

        default_hook(info);

        match report_crash(info) {
            Ok(path) => eprintln!("\nCrash report generated: {}", path.to_string_lossy()),
            Err(e) => eprintln!("\nFailed to generate crash report: {e}"),
        }
    }));
}

fn report_crash(info: &PanicHookInfo) -> Result<PathBuf, io::Error> {
    let (mut file, file_path) = crash_report_file()?;

    writeln!(file, "=== PANIC REPORT ===")?;

    let message = if let Some(msg) = info.payload().downcast_ref::<&str>() {
        *msg
    } else if let Some(msg) = info.payload().downcast_ref::<String>() {
        msg.as_str()
    } else {
        "unknown"
    };
    writeln!(file, "Message: {message}")?;

    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown".into());
    writeln!(file, "Location: {location}")?;

    let curr_thread = thread::current();
    let thread_name = curr_thread.name().unwrap_or("anonymous");
    writeln!(file, "Thread: {thread_name}")?;

    writeln!(file, "Backtrace:\n{}", Backtrace::force_capture())?;
    writeln!(file)?;

    Ok(file_path)
}

/// Get the next unique crash report file path.
fn crash_report_file() -> Result<(File, PathBuf), io::Error> {
    let file_name = OffsetDateTime::now_local()
        .ok()
        .and_then(|now| {
            now.format(time::macros::format_description!(
                "[year]-[month]-[day]_[hour]-[minute]-[second]"
            ))
            .ok()
        })
        .unwrap_or_else(|| String::from("unknown_time"));

    let mut path = local_data::crash_reports_path().join(file_name);
    for i in 1.. {
        match File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((file, path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }

        if i != 1 {
            path.pop();
        }
        path.push(format!("_{}", i));
    }
    unreachable!("1.. is infinite")
}
