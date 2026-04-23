use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_rp::{gpio::Pull, i2c};

use crate::regs::QMI8658_I2C_ADDR;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    I2c(i2c::Error),
    SharedI2c(I2cDeviceError<i2c::Error>),
    Timeout,
    InvalidAddress(u8),
    InvalidChipId(u8),
    RegisterVerify { reg: u8, expected: u8, actual: u8 },
    Ctrl9Timeout,
}

impl From<i2c::Error> for Error {
    fn from(value: i2c::Error) -> Self {
        Self::I2c(value)
    }
}

impl From<I2cDeviceError<i2c::Error>> for Error {
    fn from(value: I2cDeviceError<i2c::Error>) -> Self {
        Self::SharedI2c(value)
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
