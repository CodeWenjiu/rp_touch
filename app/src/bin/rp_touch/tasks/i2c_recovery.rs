use embassy_rp::{
    gpio::{AnyPin, Level, OutputOpenDrain},
    i2c::{self, Async, I2c},
    pac, peripherals,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

const I2C1_SDA_PIN: u8 = 6;
const I2C1_SCL_PIN: u8 = 7;
const I2C_BUS_FREQ_HZ: u32 = 400_000;

const PULSE_COUNT: usize = 9;
const BUS_DELAY_CYCLES: u32 = 1_500;
const RESET_DELAY_CYCLES: u32 = 10_000;

static I2C_RECOVERY_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

fn bus_delay() {
    for _ in 0..BUS_DELAY_CYCLES {
        core::hint::spin_loop();
    }
}

fn reset_i2c1_peripheral() {
    let resets = pac::RESETS;
    resets.reset().modify(|w| w.set_i2c1(true));
    for _ in 0..RESET_DELAY_CYCLES {
        core::hint::spin_loop();
    }
    resets.reset().modify(|w| w.set_i2c1(false));
    while !resets.reset_done().read().i2c1() {}
}

unsafe fn pulse_i2c_lines() {
    let mut sda = OutputOpenDrain::new(unsafe { AnyPin::steal(I2C1_SDA_PIN) }, Level::High);
    let mut scl = OutputOpenDrain::new(unsafe { AnyPin::steal(I2C1_SCL_PIN) }, Level::High);
    sda.set_pullup(true);
    scl.set_pullup(true);

    bus_delay();

    for _ in 0..PULSE_COUNT {
        scl.set_low();
        bus_delay();
        scl.set_high();
        bus_delay();
    }

    // Generate an explicit STOP condition.
    sda.set_low();
    bus_delay();
    scl.set_high();
    bus_delay();
    sda.set_high();
    bus_delay();
}

fn build_i2c1() -> I2c<'static, peripherals::I2C1, Async> {
    let mut cfg = i2c::Config::default();
    cfg.frequency = I2C_BUS_FREQ_HZ;
    cfg.sda_pullup = true;
    cfg.scl_pullup = true;

    I2c::new_async(
        unsafe { peripherals::I2C1::steal() },
        unsafe { peripherals::PIN_7::steal() },
        unsafe { peripherals::PIN_6::steal() },
        crate::I2cIrqs,
        cfg,
    )
}

pub async fn recover_i2c1_bus(i2c_bus: &'static crate::SharedI2c1Bus) {
    let _recover_guard = I2C_RECOVERY_LOCK.lock().await;
    let mut bus_guard = i2c_bus.lock().await;

    reset_i2c1_peripheral();
    unsafe { pulse_i2c_lines() };
    reset_i2c1_peripheral();
    *bus_guard = build_i2c1();
}
