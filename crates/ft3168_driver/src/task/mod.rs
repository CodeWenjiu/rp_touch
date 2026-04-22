use embassy_time::{Duration, Timer};

use crate::{
    device::Ft3168,
    storage::TouchPipeline,
    types::{CaptureState, TouchSample},
};

const TOUCH_CAPTURE_INTERVAL_MS: u64 = 8;

#[embassy_executor::task]
pub async fn touch_capture_task(mut touch: Ft3168<'static>, pipeline: &'static TouchPipeline) -> ! {
    match touch.init().await {
        Ok(chip_id) => {
            pipeline.set_chip_id(chip_id);
            pipeline.set_state(CaptureState::Running);
        }
        Err(_) => {
            pipeline.set_state(CaptureState::InitFailed);
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    let mut fail_count = 0u32;
    loop {
        match touch.read_touch_sample().await {
            Ok(sample) => {
                fail_count = 0;
                pipeline.set_read_fail_count(0);
                pipeline.push_sample(sample);
            }
            Err(_) => {
                fail_count = fail_count.saturating_add(1);
                pipeline.set_read_fail_count(fail_count);
                pipeline.push_sample(TouchSample::default());
            }
        }

        Timer::after(Duration::from_millis(TOUCH_CAPTURE_INTERVAL_MS)).await;
    }
}
