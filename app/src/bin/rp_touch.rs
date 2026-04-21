#![no_std]
#![no_main]

use core::fmt::Write;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use panic_probe as _;

// Program metadata for `picotool info`.
// This isn't needed, but it's recommended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let imu = qmi8658_driver::Qmi8658::new_default(p.I2C1, p.PIN_6, p.PIN_7, p.PIN_8).unwrap();
    spawner.spawn(qmi8658_driver::imu_capture_task(imu).unwrap());

    let mut class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());

    class.wait_connection().await;

    let mut buf = [0u8; 64];

    loop {
        match select(
            class.read_packet(&mut buf),
            Timer::after(Duration::from_millis(1000)),
        )
        .await
        {
            Either::First(Ok(n)) => {
                if n > 0 {
                    let _ = class.write_packet(&buf[..n]).await;
                }
            }
            Either::First(Err(_)) => {}
            Either::Second(()) => {
                if let Some(frame) = qmi8658_driver::read_latest_frame() {
                    let mut line = heapless::String::<96>::new();
                    let _ = write!(line, "{}\r\n", frame);
                    let _ = class.write_packet(line.as_bytes()).await;
                }
            }
        }
    }
}
