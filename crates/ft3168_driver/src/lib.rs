#![no_std]

mod device;
mod regs;
mod storage;
mod task;
mod types;

pub use device::{Ft3168, SharedI2cBus};
pub use regs::{
    FT3168_I2C_ADDR, FT3168_MAX_TOUCH_POINTS, FT3168_REG_CHIP_ID, FT3168_REG_FIRMWARE_ID,
    FT3168_REG_TOUCH_DATA_START, FT3168_REG_TOUCH_STATUS,
};
pub use storage::{TouchPipeline, TouchReader};
pub use task::touch_capture_task;
pub use types::{
    CaptureState, CaptureStats, Error, Ft3168Config, TouchFrame, TouchPoint, TouchSample,
};
