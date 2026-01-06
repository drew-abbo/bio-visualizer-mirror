mod args;
mod other_instances;
mod receiver;
mod sender;

use std::process::ExitCode;

use util::stop_signals;

use args::Args;
use other_instances::{InstanceLock, InstanceLockError};
use receiver::PersistedData;

const GENERIC_ERROR_MSG: &str = "Something went wrong.";

fn main() -> ExitCode {
    let args = Args::default();

    #[cfg(debug_assertions)]
    {
        use util::debug_log;
        if args.no_debug_logging {
            debug_log::disable();
        } else if !args.debug_error_log_panics {
            debug_log::panic_on_errors::disable();
        }
    }

    if let Err(e) = stop_signals::polling::enable() {
        util::debug_log_error!("Failed enable stop signal polling: {e}");
        eprintln!("{GENERIC_ERROR_MSG}");
        return ExitCode::FAILURE;
    }

    match InstanceLock::<PersistedData>::from_default() {
        Ok(instance_lock) => receiver::receiver(args, instance_lock),
        Err(InstanceLockError::Locked) => sender::sender(args),

        Err(e) => {
            util::debug_log_error!("Failed to try acquiring instance lock: {e}");
            eprintln!("{GENERIC_ERROR_MSG}");
            ExitCode::FAILURE
        }
    }
}
