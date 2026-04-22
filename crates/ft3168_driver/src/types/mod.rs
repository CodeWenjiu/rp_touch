mod capture;
mod config;
mod error;
mod touch;

pub use capture::{CaptureState, CaptureStats};
pub use config::Ft3168Config;
pub use error::Error;
pub use touch::{TouchEvent, TouchFrame, TouchPoint, TouchSample};
