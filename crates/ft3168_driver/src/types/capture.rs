#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState {
    Starting,
    Running,
    InitFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureStats {
    pub state: CaptureState,
    pub chip_id: u8,
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            state: CaptureState::Starting,
            chip_id: 0,
        }
    }
}
