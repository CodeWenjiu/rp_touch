use embassy_rp::gpio::Pull;
use i2c_bus::BusError;

use crate::regs::QMI8658_I2C_ADDR;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bus(BusError),
    InvalidAddress(u8),
    InvalidChipId(u8),
    RegisterVerify {
        reg: u8,
        expected: u8,
        actual: u8,
    },
    Ctrl9Timeout,
}

impl From<BusError> for Error {
    fn from(value: BusError) -> Self {
        Self::Bus(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Qmi8658Config {
    pub address: u8,
    pub i2c_frequency_hz: u32,
    pub int1_pull: Pull,
}

impl Default for Qmi8658Config {
    fn default() -> Self {
        Self {
            address: QMI8658_I2C_ADDR,
            i2c_frequency_hz: 400_000,
            int1_pull: Pull::Up,
        }
    }
}
