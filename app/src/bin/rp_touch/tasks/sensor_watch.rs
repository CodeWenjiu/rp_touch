use embassy_time::{Duration, Timer};

use crate::shared::{IMU_WATCH, SENSOR_WATCH_PERIOD_MS, TOUCH_WATCH};

#[embassy_executor::task]
pub async fn sensor_watch_task(
    imu_pipeline: &'static qmi8658_driver::ImuPipeline,
    touch_pipeline: &'static ft3168_driver::TouchPipeline,
) -> ! {
    let imu_reader = imu_pipeline.reader();
    let touch_reader = touch_pipeline.reader();

    let imu_sender = IMU_WATCH.sender();
    let touch_sender = TOUCH_WATCH.sender();

    let mut last_imu_frame = imu_reader.read_latest_frame();
    let mut last_touch_frame = touch_reader.read_latest_frame();
    imu_sender.send(last_imu_frame);
    touch_sender.send(last_touch_frame);

    loop {
        let imu_frame = imu_reader.read_latest_frame();
        if imu_frame != last_imu_frame {
            imu_sender.send(imu_frame);
            last_imu_frame = imu_frame;
        }

        let touch_frame = touch_reader.read_latest_frame();
        if touch_frame != last_touch_frame {
            touch_sender.send(touch_frame);
            last_touch_frame = touch_frame;
        }

        Timer::after(Duration::from_millis(SENSOR_WATCH_PERIOD_MS)).await;
    }
}
