#![no_std]
#![no_main]

#[path = "rp_touch/shared.rs"]
mod shared;
#[path = "rp_touch/slint_ui.rs"]
mod slint_ui;
#[path = "rp_touch/tasks/mod.rs"]
mod tasks;

use core::ptr::addr_of_mut;

use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    bind_interrupts,
    clocks::ClockConfig,
    i2c::{self, I2c},
    multicore::Stack,
    peripherals,
};
use embassy_time::{Duration, Timer};
use i2c_bus::{BusConfig, BusStats};
pub(crate) use i2c_bus::SharedI2c1Bus;
use panic_probe as _;
use static_cell::StaticCell;

const CORE1_STACK_SIZE: usize = 16 * 1024;
pub(crate) const SYSTEM_CLOCK_HZ: u32 = 280_000_000;
pub(crate) const SYSTEM_CLOCK_MHZ: i32 = (SYSTEM_CLOCK_HZ / 1_000_000) as i32;

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

static mut CORE1_STACK: Stack<CORE1_STACK_SIZE> = Stack::new();

static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
static IMU_PIPELINE_CELL: StaticCell<qmi8658_driver::ImuPipeline> = StaticCell::new();
static TOUCH_PIPELINE_CELL: StaticCell<ft3168_driver::TouchPipeline> = StaticCell::new();
static I2C_BUS_CELL: StaticCell<SharedI2c1Bus> = StaticCell::new();
static BUS_STATS_CELL: StaticCell<BusStats> = StaticCell::new();

bind_interrupts!(struct I2cIrqs {
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    board_alloc::init();

    let mut config = embassy_rp::config::Config::new(
        ClockConfig::system_freq(SYSTEM_CLOCK_HZ)
            .expect("failed to set system clock to configured frequency"),
    );
    config.clocks.core_voltage = embassy_rp::clocks::CoreVoltage::V1_30;
    let p = embassy_rp::init(config);

    // Core1: UI state update + Slint render + display DMA.
    embassy_rp::multicore::spawn_core1(
        p.CORE1,
        unsafe { &mut *addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(tasks::ui_render::ui_render_task().unwrap());
            });
        },
    );

    // ── I2C bus setup ─────────────────────────────────────────────────
    let mut i2c_cfg = i2c::Config::default();
    i2c_cfg.frequency = 400_000;
    i2c_cfg.sda_pullup = true;
    i2c_cfg.scl_pullup = true;
    let i2c = I2c::new_async(p.I2C1, p.PIN_7, p.PIN_6, I2cIrqs, i2c_cfg);
    let bus_config = BusConfig::default();
    let bus = SharedI2c1Bus::init(&I2C_BUS_CELL, &BUS_STATS_CELL, i2c, bus_config);

    // ── device handles ────────────────────────────────────────────────
    let imu_dev = bus.device(25, 3);
    let imu =
        qmi8658_driver::Qmi8658::new(imu_dev, p.PIN_8, qmi8658_driver::Qmi8658Config::default())
            .unwrap();

    let touch_dev = bus.device(3, 3);
    let touch =
        ft3168_driver::Ft3168::new(touch_dev, ft3168_driver::Ft3168Config::default()).unwrap();

    // ── pipelines ─────────────────────────────────────────────────────
    let imu_pipeline = IMU_PIPELINE_CELL.init(qmi8658_driver::ImuPipeline::new());
    let touch_pipeline = TOUCH_PIPELINE_CELL.init(ft3168_driver::TouchPipeline::new());

    // ── tasks ─────────────────────────────────────────────────────────
    spawner.spawn(
        tasks::sensor_hub::sensor_hub_task(imu, imu_pipeline, touch, touch_pipeline, bus).unwrap(),
    );
    spawner.spawn(
        tasks::sensor_watch::sensor_watch_task(imu_pipeline, touch_pipeline).unwrap(),
    );

    let class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());
    spawner.spawn(tasks::usb_telemetry::usb_telemetry_task(class).unwrap());
    spawner.spawn(tasks::chip_temp::chip_temp_task(p.ADC, p.ADC_TEMP_SENSOR).unwrap());

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
