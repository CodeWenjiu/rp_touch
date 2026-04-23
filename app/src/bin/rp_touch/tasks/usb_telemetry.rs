use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};

use crate::shared::{IMU_REPORT_PERIOD_MS, IMU_WATCH, TOUCH_WATCH};

#[embassy_executor::task]
pub async fn usb_telemetry_task(
    mut class: usb_serial::UsbSerialClass,
    imu_pipeline: &'static qmi8658_driver::ImuPipeline,
    touch_pipeline: &'static ft3168_driver::TouchPipeline,
) -> ! {
    let mut serial = usb_serial::UsbTextWriter::new(&mut class);
    serial.wait_connection().await;
    let _ = usb_serial::usb_println!(serial, "BOOT,display_ready");

    let mut imu_receiver = IMU_WATCH.receiver().unwrap();
    let mut touch_receiver = TOUCH_WATCH.receiver().unwrap();

    let mut latest_imu = imu_receiver.try_get().unwrap_or_default();
    let mut latest_touch = touch_receiver.try_get().unwrap_or_default();
    let mut buf = [0u8; 64];

    loop {
        match select(
            serial.read_packet(&mut buf),
            Timer::after(Duration::from_millis(IMU_REPORT_PERIOD_MS)),
        )
        .await
        {
            Either::First(Ok(n)) => {
                if n > 0 {
                    let _ = serial.write_packet(&buf[..n]).await;
                }
            }
            Either::First(Err(_)) => {}
            Either::Second(()) => {
                while let Some(frame) = imu_receiver.try_changed() {
                    latest_imu = frame;
                }
                while let Some(frame) = touch_receiver.try_changed() {
                    latest_touch = frame;
                }

                let tilt = latest_imu.sample.tilt_deg_from_accel_8g();
                let imu_stats = imu_pipeline.capture_stats();
                let touch_stats = touch_pipeline.capture_stats();

                let _ = usb_serial::usb_println!(
                    serial,
                    "{},touch={:?},imu_state={:?},touch_state={:?},touch_chip=0x{:02X}",
                    tilt,
                    latest_touch.sample,
                    imu_stats.state,
                    touch_stats.state,
                    touch_stats.chip_id
                );
            }
        }
    }
}
