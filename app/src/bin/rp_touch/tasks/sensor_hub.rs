use embassy_time::{Duration, Instant, Timer};

use super::i2c_recovery::recover_i2c1_bus;

const IMU_POLL_INTERVAL_MS: u64 = 5;
const TOUCH_POLL_INTERVAL_MS: u64 = 20;
const TOUCH_REINIT_INTERVAL_MS: u64 = 1500;
const IMU_TEMP_READ_PERIOD_MS: u64 = 2000;
const IMU_READ_ERROR_LIMIT: u8 = 6;
const TOUCH_READ_ERROR_LIMIT: u8 = 6;
const RECOVERY_COOLDOWN_MS: u64 = 300;
const HUB_IDLE_SLEEP_MS: u64 = 1;

fn imu_error_to_capture_state(err: qmi8658_driver::Error) -> qmi8658_driver::CaptureState {
    match err {
        qmi8658_driver::Error::InvalidChipId(chip_id) => {
            qmi8658_driver::CaptureState::InvalidChipId(chip_id)
        }
        _ => qmi8658_driver::CaptureState::InitFailed,
    }
}

async fn recover_with_cooldown(i2c_bus: &'static crate::SharedI2c1Bus) {
    recover_i2c1_bus(i2c_bus).await;
    Timer::after(Duration::from_millis(RECOVERY_COOLDOWN_MS)).await;
}

#[embassy_executor::task]
pub async fn sensor_hub_task(
    mut imu: qmi8658_driver::Qmi8658<'static>,
    imu_pipeline: &'static qmi8658_driver::ImuPipeline,
    mut touch: ft3168_driver::Ft3168<'static>,
    touch_pipeline: &'static ft3168_driver::TouchPipeline,
    i2c_bus: &'static crate::SharedI2c1Bus,
) -> ! {
    loop {
        imu_pipeline.set_state(qmi8658_driver::CaptureState::Starting);
        touch_pipeline.set_state(ft3168_driver::CaptureState::Starting);

        if let Err(err) = imu.init().await {
            imu_pipeline.set_state(imu_error_to_capture_state(err));
            touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
            recover_with_cooldown(i2c_bus).await;
            continue;
        }
        if let Err(err) = imu.enable_accel_gyro().await {
            imu_pipeline.set_state(imu_error_to_capture_state(err));
            touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
            recover_with_cooldown(i2c_bus).await;
            continue;
        }

        let mut touch_ready = false;
        let mut next_touch_reinit_at = Instant::now();
        match touch.init().await {
            Ok(chip_id) => {
                touch_pipeline.set_chip_id(chip_id);
                touch_pipeline.set_state(ft3168_driver::CaptureState::Running);
                touch_ready = true;
            }
            Err(_) => {
                touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                touch_pipeline.push_sample(ft3168_driver::TouchSample::default());
                next_touch_reinit_at =
                    Instant::now() + Duration::from_millis(TOUCH_REINIT_INTERVAL_MS);
            }
        }

        imu_pipeline.set_state(qmi8658_driver::CaptureState::Running);

        let mut imu_read_errors = 0u8;
        let mut touch_read_errors = 0u8;
        let mut next_imu_poll_at = Instant::now();
        let mut next_touch_poll_at = Instant::now();
        let mut last_temp_read_at = Instant::now();

        loop {
            let mut should_recover = false;
            let mut did_work = false;

            let now = Instant::now();
            if now >= next_imu_poll_at {
                did_work = true;
                next_imu_poll_at = now + Duration::from_millis(IMU_POLL_INTERVAL_MS);

                match imu.read_accel_gyro_raw().await {
                    Ok(sample) => {
                        imu_read_errors = 0;
                        imu_pipeline.push_sample(sample);
                    }
                    Err(_) => {
                        imu_read_errors = imu_read_errors.saturating_add(1);
                        if imu_read_errors >= IMU_READ_ERROR_LIMIT {
                            imu_pipeline.set_state(qmi8658_driver::CaptureState::InitFailed);
                            should_recover = true;
                        }
                    }
                }
            }

            let now = Instant::now();
            if touch_ready {
                if now >= next_touch_poll_at {
                    did_work = true;
                    next_touch_poll_at = now + Duration::from_millis(TOUCH_POLL_INTERVAL_MS);

                    match touch.read_touch_sample().await {
                        Ok(sample) => {
                            touch_read_errors = 0;
                            touch_pipeline.push_sample(sample);
                        }
                        Err(_) => {
                            touch_read_errors = touch_read_errors.saturating_add(1);
                            touch_pipeline.push_sample(ft3168_driver::TouchSample::default());
                            if touch_read_errors >= TOUCH_READ_ERROR_LIMIT {
                                touch_ready = false;
                                touch_read_errors = 0;
                                touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                                next_touch_reinit_at =
                                    Instant::now() + Duration::from_millis(TOUCH_REINIT_INTERVAL_MS);
                            }
                        }
                    }
                }
            } else if now >= next_touch_reinit_at {
                did_work = true;
                match touch.init().await {
                    Ok(chip_id) => {
                        touch_pipeline.set_chip_id(chip_id);
                        touch_pipeline.set_state(ft3168_driver::CaptureState::Running);
                        touch_ready = true;
                        touch_read_errors = 0;
                        next_touch_poll_at = Instant::now();
                    }
                    Err(_) => {
                        touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                        touch_pipeline.push_sample(ft3168_driver::TouchSample::default());
                        next_touch_reinit_at =
                            Instant::now() + Duration::from_millis(TOUCH_REINIT_INTERVAL_MS);
                    }
                }
            }

            let now = Instant::now();
            if now.saturating_duration_since(last_temp_read_at)
                >= Duration::from_millis(IMU_TEMP_READ_PERIOD_MS)
            {
                did_work = true;
                if let Ok(temp_c) = imu.read_temperature().await {
                    imu_pipeline.push_temp(temp_c);
                }
                last_temp_read_at = now;
            }

            if should_recover {
                touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                recover_with_cooldown(i2c_bus).await;
                break;
            }

            if !did_work {
                Timer::after(Duration::from_millis(HUB_IDLE_SLEEP_MS)).await;
            }
        }
    }
}
