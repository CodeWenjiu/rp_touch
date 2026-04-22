use embassy_time::{Duration, Timer};

use crate::{
    device::Qmi8658,
    storage::ImuPipeline,
    types::{CaptureState, FifoConfig, ImuRawSample, ImuReport, Int1FifoStreamState},
};

fn report_to_capture_state(report: ImuReport) -> CaptureState {
    match report {
        ImuReport::InvalidChipId(chip_id) => CaptureState::InvalidChipId(chip_id),
        ImuReport::InitError => CaptureState::InitFailed,
        _ => CaptureState::FifoConfigFailed,
    }
}

#[embassy_executor::task]
pub async fn imu_capture_task(mut imu: Qmi8658<'static>, pipeline: &'static ImuPipeline) -> ! {
    loop {
        if let Err(report) = imu.setup_int1_fifo_stream(FifoConfig::default()).await {
            pipeline.set_state(report_to_capture_state(report));
            Timer::after(Duration::from_secs(1)).await;
            continue;
        }

        let mut stream_state = Int1FifoStreamState::default();
        let mut fifo_batch = [ImuRawSample {
            accel: [0; 3],
            gyro: [0; 3],
        }; 4];
        pipeline.set_state(CaptureState::Running);

        loop {
            match imu
                .poll_int1_fifo_report(&mut stream_state, &mut fifo_batch)
                .await
            {
                Ok(n) => {
                    for sample in fifo_batch[..n].iter().copied() {
                        pipeline.push_sample(sample);
                    }
                    pipeline.set_read_fail_count(0);
                }
                Err(ImuReport::ReadError(count)) => {
                    pipeline.set_read_fail_count(count);
                }
                Err(_) => break,
            }
        }
    }
}
