use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};

use crate::types::{CaptureState, CaptureStats, ImuFrame, ImuRawSample};

mod reader;

pub const IMU_FRAME_QUEUE_CAPACITY: usize = 128;

const STATE_STARTING: u8 = 0;
const STATE_RUNNING: u8 = 1;
const STATE_INIT_FAILED: u8 = 2;
const STATE_INVALID_CHIP_ID: u8 = 3;
const STATE_FIFO_CONFIG_FAILED: u8 = 4;

pub struct ImuPipeline {
    channel: Channel<NoopRawMutex, ImuFrame, IMU_FRAME_QUEUE_CAPACITY>,
    state: AtomicU8,
    invalid_chip_id: AtomicU8,
    pushed_samples: AtomicU32,
    popped_samples: AtomicU32,
    dropped_samples: AtomicU32,
    read_fail_count: AtomicU32,
    latest_seq: AtomicU32,
    next_seq: AtomicU32,
}

impl ImuPipeline {
    pub const fn new() -> Self {
        Self {
            channel: Channel::new(),
            state: AtomicU8::new(STATE_STARTING),
            invalid_chip_id: AtomicU8::new(0),
            pushed_samples: AtomicU32::new(0),
            popped_samples: AtomicU32::new(0),
            dropped_samples: AtomicU32::new(0),
            read_fail_count: AtomicU32::new(0),
            latest_seq: AtomicU32::new(0),
            next_seq: AtomicU32::new(1),
        }
    }

    pub fn reader(&self) -> ImuReader<'_> {
        ImuReader {
            pipeline: self,
            latest: ImuFrame::default(),
        }
    }

    pub fn capture_stats(&self) -> CaptureStats {
        let state = match self.state.load(Ordering::Relaxed) {
            STATE_RUNNING => CaptureState::Running,
            STATE_INIT_FAILED => CaptureState::InitFailed,
            STATE_INVALID_CHIP_ID => {
                CaptureState::InvalidChipId(self.invalid_chip_id.load(Ordering::Relaxed))
            }
            STATE_FIFO_CONFIG_FAILED => CaptureState::FifoConfigFailed,
            _ => CaptureState::Starting,
        };

        CaptureStats {
            state,
            pushed_samples: self.pushed_samples.load(Ordering::Relaxed),
            popped_samples: self.popped_samples.load(Ordering::Relaxed),
            dropped_samples: self.dropped_samples.load(Ordering::Relaxed),
            read_fail_count: self.read_fail_count.load(Ordering::Relaxed),
            latest_seq: self.latest_seq.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn set_state(&self, state: CaptureState) {
        match state {
            CaptureState::Starting => self.state.store(STATE_STARTING, Ordering::Relaxed),
            CaptureState::Running => self.state.store(STATE_RUNNING, Ordering::Relaxed),
            CaptureState::InitFailed => self.state.store(STATE_INIT_FAILED, Ordering::Relaxed),
            CaptureState::InvalidChipId(chip_id) => {
                self.invalid_chip_id.store(chip_id, Ordering::Relaxed);
                self.state.store(STATE_INVALID_CHIP_ID, Ordering::Relaxed);
            }
            CaptureState::FifoConfigFailed => {
                self.state.store(STATE_FIFO_CONFIG_FAILED, Ordering::Relaxed)
            }
        }
    }

    pub(crate) fn push_sample(&self, sample: ImuRawSample) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let frame = ImuFrame { seq, sample };

        if self.channel.try_send(frame).is_err() {
            self.dropped_samples.fetch_add(1, Ordering::Relaxed);
        }

        self.pushed_samples.fetch_add(1, Ordering::Relaxed);
        self.latest_seq.store(seq, Ordering::Relaxed);
    }

    pub(crate) fn set_read_fail_count(&self, count: u32) {
        self.read_fail_count.store(count, Ordering::Relaxed);
    }
}

impl Default for ImuPipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ImuReader<'a> {
    pipeline: &'a ImuPipeline,
    latest: ImuFrame,
}
