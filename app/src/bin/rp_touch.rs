#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_rp::{
    bind_interrupts,
    i2c::{self, Async, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
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
static TOUCH_PIPELINE_CELL: StaticCell<ft3168_driver::TouchPipeline> = StaticCell::new();
static I2C1_BUS_CELL: StaticCell<Mutex<NoopRawMutex, I2c<'static, peripherals::I2C1, Async>>> =
    StaticCell::new();

bind_interrupts!(struct I2cIrqs {
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    board_alloc::init();

    let p = embassy_rp::init(Default::default());

    let mut display = co5300_driver::Co5300::new_default();
    display.init_default().await;
    let framebuffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER) };
    framebuffer.fill_rgb565(0x001F);
    display.write_framebuffer(framebuffer).await;

    let mut i2c_cfg = i2c::Config::default();
    i2c_cfg.frequency = 400_000;
    i2c_cfg.sda_pullup = true;
    i2c_cfg.scl_pullup = true;
    let i2c = I2c::new_async(p.I2C1, p.PIN_7, p.PIN_6, I2cIrqs, i2c_cfg);
    let i2c_bus = I2C1_BUS_CELL.init(Mutex::new(i2c));

    let imu_pipeline = IMU_PIPELINE_CELL.init(qmi8658_driver::ImuPipeline::new());
    let imu = qmi8658_driver::Qmi8658::new_shared(
        i2c_bus,
        p.PIN_8,
        qmi8658_driver::Qmi8658Config::default(),
    )
    .unwrap();
    spawner.spawn(qmi8658_driver::imu_capture_task(imu, imu_pipeline).unwrap());
    let mut imu_reader = imu_pipeline.reader();

    let touch_pipeline = TOUCH_PIPELINE_CELL.init(ft3168_driver::TouchPipeline::new());
    let touch = ft3168_driver::Ft3168::new_shared(i2c_bus, ft3168_driver::Ft3168Config::default())
        .unwrap();
    spawner.spawn(ft3168_driver::touch_capture_task(touch, touch_pipeline).unwrap());
    let mut touch_reader = touch_pipeline.reader();

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
                let touch_frame = touch_reader.read_latest_frame();
                let _ = usb_serial::usb_println!(
                    serial,
                    "{},touch={}",
                    tilt,
                    touch_frame.sample.touch_count
                );
            }
        }
    }
}
