use embassy_time::{Duration, Timer};

use crate::{
    device::Ft3168,
    storage::TouchPipeline,
    types::{CaptureState, TouchSample},
};

const TOUCH_CAPTURE_INTERVAL_MS: u64 = 8;

#[embassy_executor::task]
pub async fn touch_capture_task(mut touch: Ft3168<'static>, pipeline: &'static TouchPipeline) -> ! {
    loop {
        match touch.init().await {
            Ok(chip_id) => {
                pipeline.set_chip_id(chip_id);
                pipeline.set_state(CaptureState::Running);
                break;
            }
            Err(_) => {
                pipeline.set_state(CaptureState::InitFailed);
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    loop {
        match touch.read_touch_sample().await {
            Ok(sample) => {
                pipeline.push_sample(sample);
            }
            Err(_) => {
                pipeline.push_sample(TouchSample::default());
            }
        }

        Timer::after(Duration::from_millis(TOUCH_CAPTURE_INTERVAL_MS)).await;
    }
}
