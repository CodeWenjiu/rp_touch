#![no_std]

extern crate alloc;

mod backend;
mod constants;
mod pipeline;
mod platform;
mod touch;
mod ui_pixel;

pub use backend::SlintBackend;
