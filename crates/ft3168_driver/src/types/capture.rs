#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState {
    Starting,
    Running,
    InitFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureStats {
    pub state: CaptureState,
    pub pushed_frames: u32,
    pub popped_frames: u32,
    pub dropped_frames: u32,
    pub read_fail_count: u32,
    pub latest_seq: u32,
    pub chip_id: u8,
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            state: CaptureState::Starting,
            pushed_frames: 0,
            popped_frames: 0,
            dropped_frames: 0,
            read_fail_count: 0,
            latest_seq: 0,
            chip_id: 0,
        }
    }
}
