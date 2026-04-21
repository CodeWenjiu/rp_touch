use core::cell::RefCell;

use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};

use crate::types::{CaptureStats, ImuFrame, ImuRawSample};

const RING_CAPACITY: usize = 128;
const ZERO_SAMPLE: ImuRawSample = ImuRawSample {
    accel: [0; 3],
    gyro: [0; 3],
};
const ZERO_FRAME: ImuFrame = ImuFrame {
    seq: 0,
    sample: ZERO_SAMPLE,
};

struct SampleRing {
    frames: [ImuFrame; RING_CAPACITY],
    read: usize,
    write: usize,
    len: usize,
}

impl SampleRing {
    const fn new() -> Self {
        Self {
            frames: [ZERO_FRAME; RING_CAPACITY],
            read: 0,
            write: 0,
            len: 0,
        }
    }

    fn push(&mut self, frame: ImuFrame) -> bool {
        let mut dropped = false;
        if self.len == RING_CAPACITY {
            self.read = (self.read + 1) % RING_CAPACITY;
            self.len -= 1;
            dropped = true;
        }

        self.frames[self.write] = frame;
        self.write = (self.write + 1) % RING_CAPACITY;
        self.len += 1;
        dropped
    }

    fn pop_frames_into(&mut self, out: &mut [ImuFrame]) -> usize {
        let mut count = 0usize;
        while count < out.len() && self.len > 0 {
            out[count] = self.frames[self.read];
            self.read = (self.read + 1) % RING_CAPACITY;
            self.len -= 1;
            count += 1;
        }
        count
    }

    fn latest(&self) -> Option<ImuFrame> {
        if self.len == 0 {
            return None;
        }

        let idx = if self.write == 0 {
            RING_CAPACITY - 1
        } else {
            self.write - 1
        };
        Some(self.frames[idx])
    }
}

struct CaptureStorage {
    ring: SampleRing,
    stats: CaptureStats,
    next_seq: u32,
}

impl CaptureStorage {
    const fn new() -> Self {
        Self {
            ring: SampleRing::new(),
            stats: CaptureStats {
                state: crate::types::CaptureState::Starting,
                pushed_samples: 0,
                popped_samples: 0,
                dropped_samples: 0,
                read_fail_count: 0,
                latest_seq: None,
            },
            next_seq: 0,
        }
    }
}

static CAPTURE_STORAGE: Mutex<CriticalSectionRawMutex, RefCell<CaptureStorage>> =
    Mutex::new(RefCell::new(CaptureStorage::new()));

pub(crate) fn set_state(state: crate::types::CaptureState) {
    CAPTURE_STORAGE.lock(|storage| {
        storage.borrow_mut().stats.state = state;
    });
}

pub(crate) fn push_sample(sample: ImuRawSample) {
    CAPTURE_STORAGE.lock(|storage| {
        let mut storage = storage.borrow_mut();
        let frame = ImuFrame {
            seq: storage.next_seq,
            sample,
        };
        storage.next_seq = storage.next_seq.wrapping_add(1);

        if storage.ring.push(frame) {
            storage.stats.dropped_samples = storage.stats.dropped_samples.saturating_add(1);
        }
        storage.stats.pushed_samples = storage.stats.pushed_samples.saturating_add(1);
        storage.stats.latest_seq = Some(frame.seq);
    });
}

pub(crate) fn set_read_fail_count(count: u32) {
    CAPTURE_STORAGE.lock(|storage| {
        storage.borrow_mut().stats.read_fail_count = count;
    });
}

pub(crate) fn pop_one_frame() -> Option<ImuFrame> {
    CAPTURE_STORAGE.lock(|storage| {
        let mut storage = storage.borrow_mut();
        let mut one = [ZERO_FRAME; 1];
        let n = storage.ring.pop_frames_into(&mut one);
        if n == 0 {
            None
        } else {
            storage.stats.popped_samples = storage.stats.popped_samples.saturating_add(1);
            Some(one[0])
        }
    })
}

pub(crate) fn latest_frame() -> Option<ImuFrame> {
    CAPTURE_STORAGE.lock(|storage| storage.borrow().ring.latest())
}

pub(crate) fn capture_stats() -> CaptureStats {
    CAPTURE_STORAGE.lock(|storage| storage.borrow().stats)
}
