use core::sync::atomic::Ordering;

use embassy_sync::channel::TryReceiveError;

use crate::types::TouchFrame;

use super::TouchReader;

impl<'a> TouchReader<'a> {
    pub fn read_latest_frame(&mut self) -> TouchFrame {
        while let Ok(frame) = self.pipeline.channel.try_receive() {
            self.pipeline.popped_frames.fetch_add(1, Ordering::Relaxed);
            self.latest = frame;
        }
        self.latest
    }

    pub fn read_batch_frames<const N: usize>(&mut self) -> heapless::Vec<TouchFrame, N> {
        let mut out = heapless::Vec::<TouchFrame, N>::new();
        while out.len() < out.capacity() {
            match self.pipeline.channel.try_receive() {
                Ok(frame) => {
                    self.pipeline.popped_frames.fetch_add(1, Ordering::Relaxed);
                    self.latest = frame;
                    let _ = out.push(frame);
                }
                Err(TryReceiveError::Empty) => break,
            }
        }
        out
    }

    pub fn latest_cached(&self) -> TouchFrame {
        self.latest
    }
}
