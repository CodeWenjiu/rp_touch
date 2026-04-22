#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_rp::{
    bind_interrupts,
    i2c::{self, Async, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, watch::Watch};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;

const IMU_REPORT_PERIOD_MS: u64 = 100;
const SENSOR_WATCH_PERIOD_MS: u64 = 5;

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
static I2C1_BUS_CELL: StaticCell<
    Mutex<CriticalSectionRawMutex, I2c<'static, peripherals::I2C1, Async>>,
> = StaticCell::new();
static IMU_WATCH: Watch<CriticalSectionRawMutex, qmi8658_driver::ImuFrame, 4> = Watch::new();
static TOUCH_WATCH: Watch<CriticalSectionRawMutex, ft3168_driver::TouchFrame, 4> = Watch::new();

bind_interrupts!(struct I2cIrqs {
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::task]
async fn sensor_watch_task(
    imu_pipeline: &'static qmi8658_driver::ImuPipeline,
    touch_pipeline: &'static ft3168_driver::TouchPipeline,
) -> ! {
    let mut imu_reader = imu_pipeline.reader();
    let mut touch_reader = touch_pipeline.reader();

    let imu_sender = IMU_WATCH.sender();
    let touch_sender = TOUCH_WATCH.sender();

    let mut last_imu_seq = 0u32;
    let mut last_touch_seq = 0u32;

    loop {
        let imu_frame = imu_reader.read_latest_frame();
        if imu_frame.seq != 0 && imu_frame.seq != last_imu_seq {
            imu_sender.send(imu_frame);
            last_imu_seq = imu_frame.seq;
        }

        let touch_frame = touch_reader.read_latest_frame();
        if touch_frame.seq != 0 && touch_frame.seq != last_touch_seq {
            touch_sender.send(touch_frame);
            last_touch_seq = touch_frame.seq;
        }

        Timer::after(Duration::from_millis(SENSOR_WATCH_PERIOD_MS)).await;
    }
}

#[embassy_executor::task]
async fn usb_telemetry_task(
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
                    "{},touch={:?},imu_state={:?},imu_seq={},imu_fail={},imu_drop={},touch_state={:?},touch_seq={},touch_fail={},touch_drop={},touch_chip=0x{:02X}",
                    tilt,
                    latest_touch.sample,
                    imu_stats.state,
                    imu_stats.latest_seq,
                    imu_stats.read_fail_count,
                    imu_stats.dropped_samples,
                    touch_stats.state,
                    touch_stats.latest_seq,
                    touch_stats.read_fail_count,
                    touch_stats.dropped_frames,
                    touch_stats.chip_id
                );
            }
        }
    }
}

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

    let touch_pipeline = TOUCH_PIPELINE_CELL.init(ft3168_driver::TouchPipeline::new());
    let touch =
        ft3168_driver::Ft3168::new_shared(i2c_bus, ft3168_driver::Ft3168Config::default()).unwrap();
    spawner.spawn(ft3168_driver::touch_capture_task(touch, touch_pipeline).unwrap());

    spawner.spawn(sensor_watch_task(imu_pipeline, touch_pipeline).unwrap());

    let class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());
    spawner.spawn(usb_telemetry_task(class, imu_pipeline, touch_pipeline).unwrap());

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
