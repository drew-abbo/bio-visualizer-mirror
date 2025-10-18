use std::{thread, time::Duration};
use util::Mailbox;
use util::messages::{UiToMedia, MediaToUi};
use ui::{start_ui_listener, send_load, send_extract, send_shutdown};
use media::start_media_worker;

fn main() {
    // Create message channels
    let (ui_to_media, media_inbox) = Mailbox::<UiToMedia>::new_pair(64);
    let (media_to_ui, ui_inbox) = Mailbox::<MediaToUi>::new_pair(64);

    // Start workers
    let media_handle = start_media_worker(media_inbox, media_to_ui.clone());
    let ui_handle = start_ui_listener(ui_inbox);

    // Simulate a few UI actions
    send_load(&ui_to_media, "sample.mp4");
    send_extract(&ui_to_media);

    thread::sleep(Duration::from_secs(2));

    // Shut down
    send_shutdown(&ui_to_media);
    drop(ui_to_media); // closes channel
    let _ = media_handle.join();
    let _ = ui_handle.join();
}