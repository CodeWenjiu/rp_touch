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
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            state: CaptureState::Starting,
        }
    }
}
