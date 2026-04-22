use crate::config::{
    DEFAULT_CMD_PREFIX, DEFAULT_DATA_PREFIX, DEFAULT_ROWS_PER_BURST, DEFAULT_WINDOW_X_OFFSET,
    DEFAULT_WINDOW_Y_OFFSET,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Co5300Tuning {
    pub cmd_prefix: u8,
    pub data_prefix: u8,
    pub x_offset: u16,
    pub y_offset: u16,
    pub rows_per_burst: usize,
}

impl Default for Co5300Tuning {
    fn default() -> Self {
        Self {
            cmd_prefix: DEFAULT_CMD_PREFIX,
            data_prefix: DEFAULT_DATA_PREFIX,
            x_offset: DEFAULT_WINDOW_X_OFFSET,
            y_offset: DEFAULT_WINDOW_Y_OFFSET,
            rows_per_burst: DEFAULT_ROWS_PER_BURST,
        }
    }
}
