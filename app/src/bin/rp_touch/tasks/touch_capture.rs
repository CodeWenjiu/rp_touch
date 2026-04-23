use embassy_time::{Duration, Timer};

use super::i2c_recovery::recover_i2c1_bus;

const TOUCH_CAPTURE_INTERVAL_MS: u64 = 8;
const TOUCH_READ_ERROR_LIMIT: u8 = 8;
const TOUCH_REINIT_DELAY_MS: u64 = 50;

#[embassy_executor::task]
pub async fn touch_capture_task(
    mut touch: ft3168_driver::Ft3168<'static>,
    pipeline: &'static ft3168_driver::TouchPipeline,
    i2c_bus: &'static crate::SharedI2c1Bus,
) -> ! {
    loop {
        match touch.init().await {
            Ok(chip_id) => {
                pipeline.set_chip_id(chip_id);
                pipeline.set_state(ft3168_driver::CaptureState::Running);
            }
            Err(_) => {
                pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                recover_i2c1_bus(i2c_bus).await;
                Timer::after(Duration::from_millis(TOUCH_REINIT_DELAY_MS)).await;
                continue;
            }
        }

        let mut consecutive_read_errors = 0u8;

        loop {
            match touch.read_touch_sample().await {
                Ok(sample) => {
                    consecutive_read_errors = 0;
                    pipeline.push_sample(sample);
                }
                Err(_) => {
                    consecutive_read_errors = consecutive_read_errors.saturating_add(1);
                    pipeline.push_sample(ft3168_driver::TouchSample::default());

                    if consecutive_read_errors >= TOUCH_READ_ERROR_LIMIT {
                        pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                        recover_i2c1_bus(i2c_bus).await;
                        break;
                    }
                }
            }

            Timer::after(Duration::from_millis(TOUCH_CAPTURE_INTERVAL_MS)).await;
        }

        Timer::after(Duration::from_millis(TOUCH_REINIT_DELAY_MS)).await;
    }
}
