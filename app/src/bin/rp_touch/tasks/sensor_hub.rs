use embassy_time::{Duration, Instant, Timer};

use super::i2c_recovery::recover_i2c1_bus;

const IMU_FIFO_BATCH_SIZE: usize = 4;
const IMU_POLL_INTERVAL_MS: u64 = 2;
const TOUCH_POLL_INTERVAL_MS: u64 = 12;
const IMU_TEMP_READ_PERIOD_MS: u64 = 2000;
const IMU_READ_ERROR_LIMIT: u8 = 6;
const TOUCH_READ_ERROR_LIMIT: u8 = 6;
const RECOVERY_COOLDOWN_MS: u64 = 300;
const HUB_IDLE_SLEEP_MS: u64 = 1;

fn report_to_capture_state(report: qmi8658_driver::ImuReport) -> qmi8658_driver::CaptureState {
    match report {
        qmi8658_driver::ImuReport::InvalidChipId(chip_id) => {
            qmi8658_driver::CaptureState::InvalidChipId(chip_id)
        }
        qmi8658_driver::ImuReport::InitError => qmi8658_driver::CaptureState::InitFailed,
        _ => qmi8658_driver::CaptureState::FifoConfigFailed,
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
    let mut stream_state = qmi8658_driver::Int1FifoStreamState::default();
    let mut fifo_batch = [qmi8658_driver::ImuRawSample {
        accel: [0; 3],
        gyro: [0; 3],
    }; IMU_FIFO_BATCH_SIZE];

    loop {
        imu_pipeline.set_state(qmi8658_driver::CaptureState::Starting);
        touch_pipeline.set_state(ft3168_driver::CaptureState::Starting);

        if let Err(report) = imu
            .setup_int1_fifo_stream(qmi8658_driver::FifoConfig::default())
            .await
        {
            imu_pipeline.set_state(report_to_capture_state(report));
            touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
            recover_with_cooldown(i2c_bus).await;
            continue;
        }

        match touch.init().await {
            Ok(chip_id) => {
                touch_pipeline.set_chip_id(chip_id);
            }
            Err(_) => {
                imu_pipeline.set_state(qmi8658_driver::CaptureState::InitFailed);
                touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                recover_with_cooldown(i2c_bus).await;
                continue;
            }
        }

        imu_pipeline.set_state(qmi8658_driver::CaptureState::Running);
        touch_pipeline.set_state(ft3168_driver::CaptureState::Running);

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

                match imu
                    .poll_int1_fifo_report(&mut stream_state, &mut fifo_batch)
                    .await
                {
                    Ok(n) => {
                        imu_read_errors = 0;
                        for sample in fifo_batch[..n].iter().copied() {
                            imu_pipeline.push_sample(sample);
                        }
                    }
                    Err(qmi8658_driver::ImuReport::ReadError) => {
                        imu_read_errors = imu_read_errors.saturating_add(1);
                        if imu_read_errors >= IMU_READ_ERROR_LIMIT {
                            imu_pipeline.set_state(qmi8658_driver::CaptureState::InitFailed);
                            should_recover = true;
                        }
                    }
                    Err(report) => {
                        imu_pipeline.set_state(report_to_capture_state(report));
                        should_recover = true;
                    }
                }
            }

            let now = Instant::now();
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
                            touch_pipeline.set_state(ft3168_driver::CaptureState::InitFailed);
                            should_recover = true;
                        }
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
