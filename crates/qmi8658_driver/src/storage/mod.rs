use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

use crate::types::{CaptureState, CaptureStats, ImuRawSample};

mod reader;

const STATE_STARTING: u8 = 0;
const STATE_RUNNING: u8 = 1;
const STATE_INIT_FAILED: u8 = 2;
const STATE_INVALID_CHIP_ID: u8 = 3;
const STATE_FIFO_CONFIG_FAILED: u8 = 4;

pub struct ImuPipeline {
    state: AtomicU8,
    invalid_chip_id: AtomicU8,
    accel_x: AtomicI32,
    accel_y: AtomicI32,
    accel_z: AtomicI32,
    gyro_x: AtomicI32,
    gyro_y: AtomicI32,
    gyro_z: AtomicI32,
    temp_c: AtomicI32,
}

impl ImuPipeline {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_STARTING),
            invalid_chip_id: AtomicU8::new(0),
            accel_x: AtomicI32::new(0),
            accel_y: AtomicI32::new(0),
            accel_z: AtomicI32::new(0),
            gyro_x: AtomicI32::new(0),
            gyro_y: AtomicI32::new(0),
            gyro_z: AtomicI32::new(0),
            temp_c: AtomicI32::new(0),
        }
    }

    pub fn reader(&self) -> ImuReader<'_> {
        ImuReader { pipeline: self }
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

        CaptureStats { state }
    }

    pub fn set_state(&self, state: CaptureState) {
        match state {
            CaptureState::Starting => self.state.store(STATE_STARTING, Ordering::Relaxed),
            CaptureState::Running => self.state.store(STATE_RUNNING, Ordering::Relaxed),
            CaptureState::InitFailed => self.state.store(STATE_INIT_FAILED, Ordering::Relaxed),
            CaptureState::InvalidChipId(chip_id) => {
                self.invalid_chip_id.store(chip_id, Ordering::Relaxed);
                self.state.store(STATE_INVALID_CHIP_ID, Ordering::Relaxed);
            }
            CaptureState::FifoConfigFailed => self
                .state
                .store(STATE_FIFO_CONFIG_FAILED, Ordering::Relaxed),
        }
    }

    pub fn push_sample(&self, sample: ImuRawSample) {
        self.accel_x
            .store(sample.accel[0] as i32, Ordering::Relaxed);
        self.accel_y
            .store(sample.accel[1] as i32, Ordering::Relaxed);
        self.accel_z
            .store(sample.accel[2] as i32, Ordering::Relaxed);
        self.gyro_x.store(sample.gyro[0] as i32, Ordering::Relaxed);
        self.gyro_y.store(sample.gyro[1] as i32, Ordering::Relaxed);
        self.gyro_z.store(sample.gyro[2] as i32, Ordering::Relaxed);
    }

    pub fn push_temp(&self, temp_c: i32) {
        self.temp_c.store(temp_c, Ordering::Relaxed);
    }

    pub(crate) fn latest_temp(&self) -> i32 {
        self.temp_c.load(Ordering::Relaxed)
    }

    pub(crate) fn latest_sample(&self) -> ImuRawSample {
        ImuRawSample {
            accel: [
                self.accel_x.load(Ordering::Relaxed) as i16,
                self.accel_y.load(Ordering::Relaxed) as i16,
                self.accel_z.load(Ordering::Relaxed) as i16,
            ],
            gyro: [
                self.gyro_x.load(Ordering::Relaxed) as i16,
                self.gyro_y.load(Ordering::Relaxed) as i16,
                self.gyro_z.load(Ordering::Relaxed) as i16,
            ],
        }
    }
}

impl Default for ImuPipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ImuReader<'a> {
    pipeline: &'a ImuPipeline,
}
