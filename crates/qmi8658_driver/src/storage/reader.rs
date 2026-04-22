use core::sync::atomic::Ordering;

use embassy_sync::channel::TryReceiveError;

use crate::types::ImuFrame;

use super::ImuReader;

impl<'a> ImuReader<'a> {
    pub fn read_latest_frame(&mut self) -> ImuFrame {
        while let Ok(frame) = self.pipeline.channel.try_receive() {
            self.pipeline.popped_samples.fetch_add(1, Ordering::Relaxed);
            self.latest = frame;
        }
        self.latest
    }

    pub fn read_batch_frames<const N: usize>(&mut self) -> heapless::Vec<ImuFrame, N> {
        let mut out = heapless::Vec::<ImuFrame, N>::new();
        while out.len() < out.capacity() {
            match self.pipeline.channel.try_receive() {
                Ok(frame) => {
                    self.pipeline.popped_samples.fetch_add(1, Ordering::Relaxed);
                    self.latest = frame;
                    let _ = out.push(frame);
                }
                Err(TryReceiveError::Empty) => break,
            }
        }
        out
    }

    pub fn latest_cached(&self) -> ImuFrame {
        self.latest
    }
}
