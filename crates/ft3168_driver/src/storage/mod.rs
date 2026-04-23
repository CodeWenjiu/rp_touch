use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use crate::types::{CaptureState, CaptureStats, TouchSample};

mod reader;

const STATE_STARTING: u8 = 0;
const STATE_RUNNING: u8 = 1;
const STATE_INIT_FAILED: u8 = 2;

pub struct TouchPipeline {
    state: AtomicU8,
    chip_id: AtomicU8,
    latest_active: AtomicU8,
    latest_x: AtomicU32,
    latest_y: AtomicU32,
}

impl TouchPipeline {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_STARTING),
            chip_id: AtomicU8::new(0),
            latest_active: AtomicU8::new(0),
            latest_x: AtomicU32::new(0),
            latest_y: AtomicU32::new(0),
        }
    }

    pub fn reader(&self) -> TouchReader<'_> {
        TouchReader { pipeline: self }
    }

    pub fn capture_stats(&self) -> CaptureStats {
        let state = match self.state.load(Ordering::Relaxed) {
            STATE_RUNNING => CaptureState::Running,
            STATE_INIT_FAILED => CaptureState::InitFailed,
            _ => CaptureState::Starting,
        };

        CaptureStats {
            state,
            chip_id: self.chip_id.load(Ordering::Relaxed),
        }
    }

    pub fn set_state(&self, state: CaptureState) {
        match state {
            CaptureState::Starting => self.state.store(STATE_STARTING, Ordering::Relaxed),
            CaptureState::Running => self.state.store(STATE_RUNNING, Ordering::Relaxed),
            CaptureState::InitFailed => self.state.store(STATE_INIT_FAILED, Ordering::Relaxed),
        }
    }

    pub fn set_chip_id(&self, chip_id: u8) {
        self.chip_id.store(chip_id, Ordering::Relaxed);
    }

    pub fn push_sample(&self, sample: TouchSample) {
        match sample {
            Some(p) => {
                self.latest_x.store(p.x as u32, Ordering::Relaxed);
                self.latest_y.store(p.y as u32, Ordering::Relaxed);
                self.latest_active.store(1, Ordering::Relaxed);
            }
            None => {
                self.latest_active.store(0, Ordering::Relaxed);
            }
        }
    }

    pub(crate) fn latest_sample(&self) -> TouchSample {
        if self.latest_active.load(Ordering::Relaxed) == 0 {
            return None;
        }

        Some(crate::types::TouchPoint {
            x: self.latest_x.load(Ordering::Relaxed) as u16,
            y: self.latest_y.load(Ordering::Relaxed) as u16,
        })
    }
}

impl Default for TouchPipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TouchReader<'a> {
    pipeline: &'a TouchPipeline,
}
