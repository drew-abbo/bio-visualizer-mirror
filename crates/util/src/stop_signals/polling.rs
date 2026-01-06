//! Tools for handling stop signals (e.g. `SIGINT`) with polling. This allows
//! you to essentially ignore stop signals until you want to deal with them
//! (which can make resource cleanup a lot easier).

use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use signal_hook::{SigId, consts, low_level};

/// Enables the polling of stop signals (e.g. `SIGINT`) so that you can call
/// [poll]/[consume] to see if a stop signal has been sent. Also see [disable].
pub fn enable() -> Result<(), io::Error> {
    let mut sig_ids = SIG_IDS.lock().expect(super::THREAD_EXPECT_MSG);
    if sig_ids.is_some() {
        return Ok(());
    }

    *sig_ids = Some(
        super::try_array_from_fn(|i| {
            // SAFETY: Messing with atomics is one of the only things you can
            // safely do in a signal handler and that's all we're doing here.
            // There's no mutexes, no memory allocations, no functions being
            // called that aren't async-signal-safe, and nothing that can panic.
            unsafe {
                low_level::register(consts::TERM_SIGNALS[i], || {
                    STOP_SIGNALS.fetch_add(1, Ordering::SeqCst);
                })
            }
        })
        .inspect_err(|e| crate::debug_log_error!("Failed to register signal handler: {e}"))?,
    );

    Ok(())
}

/// Disables stop signal polling if stop signal polling is enabled (see
/// [enable]). [poll]/[consume] will continue to return `true` after this is
/// called if there are unconsumed stop signals.
pub fn disable() {
    let mut sig_ids = SIG_IDS.lock().expect(super::THREAD_EXPECT_MSG);
    let Some(sig_ids_inner) = sig_ids.as_ref() else {
        return;
    };

    for sig_id in sig_ids_inner.iter().cloned() {
        low_level::unregister(sig_id);
    }

    *sig_ids = None;
}

/// Returns whether stop signal polling has been enabled or not (see [enable]
/// and [disable]).
pub fn is_enabled() -> bool {
    SIG_IDS.lock().expect(super::THREAD_EXPECT_MSG).is_some()
}

/// Returns whether a stop signal (e.g. `SIGINT`) has been captured, consuming
/// the signal in the process. To poll without consuming the signal, see [poll].
///
/// This function will always return `false` if all stop signals have been
/// consumed and polling is disabled (which it is by default). See [enable] and
/// [disable].
///
/// Also see [consume_all].
pub fn consume() -> bool {
    STOP_SIGNALS
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
            (count > 0).then(|| count - 1)
        })
        .is_ok()
}

/// Like [consume] but it returns the number of signals that have been captured,
/// consuming all of them in the process.
pub fn consume_all() -> usize {
    STOP_SIGNALS.swap(0, Ordering::SeqCst)
}

/// Returns whether a stop signal (e.g. `SIGINT`) has been captured without
/// consuming the signal in the process. To consume the signal, see [consume].
///
/// This function will always return `false` if all stop signals have been
/// consumed and polling is disabled (which it is by default). See [enable] and
/// [disable].
pub fn poll() -> bool {
    STOP_SIGNALS.load(Ordering::SeqCst) > 0
}

static STOP_SIGNALS: AtomicUsize = AtomicUsize::new(0);

static SIG_IDS: Mutex<Option<[SigId; consts::TERM_SIGNALS.len()]>> = Mutex::new(None);
