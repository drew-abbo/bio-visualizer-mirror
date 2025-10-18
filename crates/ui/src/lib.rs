use std::thread;
use util::Mailbox;
use util::messages::{UiToMedia, MediaToUi};

pub fn start_ui_listener(inbox: Mailbox<MediaToUi>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(msg) = inbox.recv() {
            match msg {
                MediaToUi::Status(s) => println!("[UI] status: {s}"),
                MediaToUi::Progress { done, total } => println!("[UI] progress: {done}/{total}"),
                MediaToUi::Finished => println!("[UI] finished !!!"),
                MediaToUi::Error(e) => eprintln!("[UI] ERROR: {e}"),
            }
        }
    })
}

pub fn send_load(ui_to_media: &Mailbox<UiToMedia>, path: &str) {
    let _ = ui_to_media.send(UiToMedia::LoadVideo(path.into()));
}

pub fn send_extract(ui_to_media: &Mailbox<UiToMedia>) {
    let _ = ui_to_media.send(UiToMedia::ExtractAllFrames);
}

pub fn send_shutdown(ui_to_media: &Mailbox<UiToMedia>) {
    let _ = ui_to_media.send(UiToMedia::Shutdown);
}