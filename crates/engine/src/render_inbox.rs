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
        let mut count = 0;
        
        loop {
            match self.inbox.check_non_blocking() {
                Ok(Some(f)) => {
                    count += 1;
                    
                    // Push EVERY frame to the store (FrameStore handles overflow)
                    store.push(Arc::new(RgbaFrame { 
                        pixels: f.pixels, 
                        ..f 
                    }));
                }
                Ok(None) => break,
                Err(e) => {
                    log::warn!("inbox error: {e:?}");
                    break;
                }
            }
        }
        
        if count > 0 {
            log::info!("Drained {} frames from inbox, store now has {} frames", 
                      count, store.len());
            playback.on_new_frame(store);
        }
    }
}