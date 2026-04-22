#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState {
    Starting,
    Running,
    InitFailed,
    InvalidChipId(u8),
    FifoConfigFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureStats {
    pub state: CaptureState,
    pub pushed_samples: u32,
    pub popped_samples: u32,
    pub dropped_samples: u32,
    pub read_fail_count: u32,
    pub latest_seq: u32,
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            state: CaptureState::Starting,
            pushed_samples: 0,
            popped_samples: 0,
            dropped_samples: 0,
            read_fail_count: 0,
            latest_seq: 0,
        }
    }
}
