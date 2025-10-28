use crate::{frame_store::FrameStore, playback::Playback, types::RgbaFrame};
use std::sync::Arc;
use util::channels::message_channel::Inbox;

pub struct RenderInbox {
    inbox: Inbox<RgbaFrame>,
}

impl RenderInbox {
    pub fn new(inbox: Inbox<RgbaFrame>) -> Self {
        Self { inbox }
    }

    pub fn drain(&mut self, store: &mut FrameStore, playback: &mut Playback) {
        let mut last: Option<RgbaFrame> = None;

        loop {
            match self.inbox.check_non_blocking() {
                Ok(Some(f)) => {
                    last = Some(f);
                }
                Ok(None) => break,
                Err(_) => {
                    break;
                }
            }
        }

        // If we received any frames, push only the last one to the store
        if let Some(f) = last {
            store.push(Arc::new(RgbaFrame {
                pixels: f.pixels,
                ..f
            }));

            playback.on_new_frame(store);
        }
    }
}
