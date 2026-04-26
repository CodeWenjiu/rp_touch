use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use rp_telemetry::TelemetryFrame;

use crate::shared::{IMU_REPORT_PERIOD_MS, IMU_WATCH};

#[embassy_executor::task]
pub async fn usb_telemetry_task(mut class: usb_serial::UsbSerialClass) -> ! {
    let mut serial = usb_serial::UsbTextWriter::new(&mut class);
    serial.wait_connection().await;
    let _ = usb_serial::usb_println!(serial, "BOOT,display_ready");

    let mut imu_receiver = IMU_WATCH.receiver().unwrap();
    let mut latest_imu = imu_receiver.try_get().unwrap_or_default();
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

                let frame = TelemetryFrame::new(latest_imu.sample.accel, latest_imu.sample.gyro);
                if let Ok(line) = frame.format::<96>() {
                    let _ = usb_serial::usb_println!(serial, "{line}");
                }
            }
        }
    }
}
