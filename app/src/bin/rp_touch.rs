#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;

const IMU_REPORT_PERIOD_MS: u64 = 100;

// Program metadata for `picotool info`.
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

static mut DISPLAY_FRAMEBUFFER: co5300_driver::FrameBuffer = co5300_driver::FrameBuffer::new();
static IMU_PIPELINE_CELL: StaticCell<qmi8658_driver::ImuPipeline> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    board_alloc::init();

    let p = embassy_rp::init(Default::default());

    let mut display = co5300_driver::Co5300::new_default();
    display.init_default().await;
    let framebuffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER) };
    framebuffer.fill_rgb565(0x001F);
    display.write_framebuffer(framebuffer).await;

    let imu_pipeline = IMU_PIPELINE_CELL.init(qmi8658_driver::ImuPipeline::new());
    let imu = qmi8658_driver::Qmi8658::new_default().unwrap();
    spawner.spawn(qmi8658_driver::imu_capture_task(imu, imu_pipeline).unwrap());
    let mut imu_reader = imu_pipeline.reader();

    let mut class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());
    let mut serial = usb_serial::UsbTextWriter::new(&mut class);
    serial.wait_connection().await;
    let _ = usb_serial::usb_println!(serial, "BOOT,display_ready");

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
                let frame = imu_reader.read_latest_frame();
                let tilt = frame.sample.tilt_deg_from_accel_8g();
                let _ = usb_serial::usb_println!(serial, "{}", tilt);
            }
        }
    }
}
