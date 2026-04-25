use embassy_time::{Duration, Instant, Timer};

use super::i2c_recovery::recover_i2c1_bus;

const IMU_FIFO_BATCH_SIZE: usize = 4;
const IMU_READ_ERROR_LIMIT: u8 = 6;
const IMU_READ_ERROR_BACKOFF_MS: u64 = 2;
const IMU_REINIT_DELAY_MS: u64 = 50;
const IMU_TEMP_READ_PERIOD_MS: u64 = 2000;

fn report_to_capture_state(report: qmi8658_driver::ImuReport) -> qmi8658_driver::CaptureState {
    match report {
        qmi8658_driver::ImuReport::InvalidChipId(chip_id) => {
            qmi8658_driver::CaptureState::InvalidChipId(chip_id)
        }
        qmi8658_driver::ImuReport::InitError => qmi8658_driver::CaptureState::InitFailed,
        _ => qmi8658_driver::CaptureState::FifoConfigFailed,
    }
}

#[embassy_executor::task]
pub async fn imu_capture_task(
    mut imu: qmi8658_driver::Qmi8658<'static>,
    pipeline: &'static qmi8658_driver::ImuPipeline,
    i2c_bus: &'static crate::SharedI2c1Bus,
) -> ! {
    loop {
        if let Err(report) = imu
            .setup_int1_fifo_stream(qmi8658_driver::FifoConfig::default())
            .await
        {
            pipeline.set_state(report_to_capture_state(report));
            recover_i2c1_bus(i2c_bus).await;
            Timer::after(Duration::from_millis(IMU_REINIT_DELAY_MS)).await;
            continue;
        }

        let mut stream_state = qmi8658_driver::Int1FifoStreamState::default();
        let mut fifo_batch = [qmi8658_driver::ImuRawSample {
            accel: [0; 3],
            gyro: [0; 3],
        }; IMU_FIFO_BATCH_SIZE];
        let mut consecutive_read_errors = 0u8;
        let mut last_temp_read_at = Instant::now();

        pipeline.set_state(qmi8658_driver::CaptureState::Running);

        loop {
            match imu
                .poll_int1_fifo_report(&mut stream_state, &mut fifo_batch)
                .await
            {
                Ok(n) => {
                    consecutive_read_errors = 0;
                    for sample in fifo_batch[..n].iter().copied() {
                        pipeline.push_sample(sample);
                    }

                    let now = Instant::now();
                    if now.saturating_duration_since(last_temp_read_at)
                        >= Duration::from_millis(IMU_TEMP_READ_PERIOD_MS)
                    {
                        if let Ok(temp_c) = imu.read_temperature().await {
                            pipeline.push_temp(temp_c);
                        }
                        last_temp_read_at = now;
                    }
                }
                Err(qmi8658_driver::ImuReport::ReadError) => {
                    consecutive_read_errors = consecutive_read_errors.saturating_add(1);
                    if consecutive_read_errors >= IMU_READ_ERROR_LIMIT {
                        pipeline.set_state(qmi8658_driver::CaptureState::InitFailed);
                        recover_i2c1_bus(i2c_bus).await;
                        break;
                    }

                    Timer::after(Duration::from_millis(IMU_READ_ERROR_BACKOFF_MS)).await;
                }
                Err(report) => {
                    pipeline.set_state(report_to_capture_state(report));
                    recover_i2c1_bus(i2c_bus).await;
                    break;
                }
            }
        }

        Timer::after(Duration::from_millis(IMU_REINIT_DELAY_MS)).await;
    }
}
