//! Defines [sender], the code path that will run when this instance is not the
//! main one.

use std::process::ExitCode;

use crate::args::{Args, ForcibleFlag};
use crate::other_instances::{OIMsg, OIMsgSender};

/// The code path for when this isn't the main instance.
pub fn sender(args: Args) -> ExitCode {
    if let Some(required) = args.receive_only {
        return match required {
            ForcibleFlag::Force => {
                eprintln!("A main instance is already receiving messages.");
                ExitCode::FAILURE
            }
            ForcibleFlag::True => ExitCode::SUCCESS,
        };
    }

    let mut msg_sender = match OIMsgSender::new() {
        Ok(msg_sender) => msg_sender,
        Err(e) => {
            util::debug_log_error!("Failed to create other instance message sender: {e}");

            return match args.send_only {
                Some(ForcibleFlag::Force) => {
                    eprintln!("Failed to connect to a main instance's message receiver.");
                    ExitCode::FAILURE
                }
                Some(ForcibleFlag::True) | None => ExitCode::SUCCESS,
            };
        }
    };

    let mut exit_code = ExitCode::SUCCESS;

    let mut send = |msg| match msg_sender.send(msg) {
        Ok(_) => {
            util::debug_log_info!("Other instance message sent: `{msg}`.");
        }
        Err(e) => {
            util::debug_log_error!("Failed to send message to other instance (ignoring): {e}");

            if exit_code == ExitCode::SUCCESS {
                eprintln!("Failed to send message to other instance.");
                exit_code = ExitCode::FAILURE;
            }
        }
    };

    if !args.no_focus {
        send(OIMsg::Focus);
    }
    if args.rescan_projects {
        send(OIMsg::ProjectUpdated);
    }
    if args.project_open_failed {
        send(OIMsg::ProjectOpenFailed);
    }

    exit_code
}
