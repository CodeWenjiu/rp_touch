use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};

use crate::types::{CaptureState, CaptureStats, TouchFrame, TouchSample};

mod reader;

pub const TOUCH_FRAME_QUEUE_CAPACITY: usize = 64;

const STATE_STARTING: u8 = 0;
const STATE_RUNNING: u8 = 1;
const STATE_INIT_FAILED: u8 = 2;

pub struct TouchPipeline {
    channel: Channel<NoopRawMutex, TouchFrame, TOUCH_FRAME_QUEUE_CAPACITY>,
    state: AtomicU8,
    chip_id: AtomicU8,
    pushed_frames: AtomicU32,
    popped_frames: AtomicU32,
    dropped_frames: AtomicU32,
    read_fail_count: AtomicU32,
    latest_seq: AtomicU32,
    next_seq: AtomicU32,
}

impl TouchPipeline {
    pub const fn new() -> Self {
        Self {
            channel: Channel::new(),
            state: AtomicU8::new(STATE_STARTING),
            chip_id: AtomicU8::new(0),
            pushed_frames: AtomicU32::new(0),
            popped_frames: AtomicU32::new(0),
            dropped_frames: AtomicU32::new(0),
            read_fail_count: AtomicU32::new(0),
            latest_seq: AtomicU32::new(0),
            next_seq: AtomicU32::new(1),
        }
    }

    pub fn reader(&self) -> TouchReader<'_> {
        TouchReader {
            pipeline: self,
            latest: TouchFrame::default(),
        }
    }

    pub fn capture_stats(&self) -> CaptureStats {
        let state = match self.state.load(Ordering::Relaxed) {
            STATE_RUNNING => CaptureState::Running,
            STATE_INIT_FAILED => CaptureState::InitFailed,
            _ => CaptureState::Starting,
        };

        CaptureStats {
            state,
            pushed_frames: self.pushed_frames.load(Ordering::Relaxed),
            popped_frames: self.popped_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            read_fail_count: self.read_fail_count.load(Ordering::Relaxed),
            latest_seq: self.latest_seq.load(Ordering::Relaxed),
            chip_id: self.chip_id.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn set_state(&self, state: CaptureState) {
        match state {
            CaptureState::Starting => self.state.store(STATE_STARTING, Ordering::Relaxed),
            CaptureState::Running => self.state.store(STATE_RUNNING, Ordering::Relaxed),
            CaptureState::InitFailed => self.state.store(STATE_INIT_FAILED, Ordering::Relaxed),
        }
    }

    pub(crate) fn set_chip_id(&self, chip_id: u8) {
        self.chip_id.store(chip_id, Ordering::Relaxed);
    }

    pub(crate) fn push_sample(&self, sample: TouchSample) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let frame = TouchFrame { seq, sample };

        if self.channel.try_send(frame).is_err() {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
        }

        self.pushed_frames.fetch_add(1, Ordering::Relaxed);
        self.latest_seq.store(seq, Ordering::Relaxed);
    }

    pub(crate) fn set_read_fail_count(&self, count: u32) {
        self.read_fail_count.store(count, Ordering::Relaxed);
    }
}

impl Default for TouchPipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TouchReader<'a> {
    pipeline: &'a TouchPipeline,
    latest: TouchFrame,
}
