use crate::types::RgbaFrame;
use std::{collections::VecDeque, sync::Arc};

pub struct FrameStore {
    buf: VecDeque<Arc<RgbaFrame>>,
    cap: usize,
}

// in the future we will have to handle live streaming. So when we reach a capacity, we drop the oldest frames and store them if need be
impl FrameStore {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, f: Arc<RgbaFrame>) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(f);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn get(&self, idx: usize) -> Option<Arc<RgbaFrame>> {
        self.buf.get(idx).cloned()
    }

    pub fn newest_index(&self) -> Option<usize> {
        (!self.buf.is_empty()).then(|| self.buf.len() - 1)
    }
}
