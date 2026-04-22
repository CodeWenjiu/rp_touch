#![no_std]

mod config;
mod device;
mod framebuffer;
mod tuning;

pub use config::MAX_STABLE_SCLK_HZ;
pub use device::Co5300;
pub use framebuffer::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, DrawError, FRAMEBUFFER_BYTES, FrameBuffer, PIXEL_COUNT,
};
pub use tuning::Co5300Tuning;
