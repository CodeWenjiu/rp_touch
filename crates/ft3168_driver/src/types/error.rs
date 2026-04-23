use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_rp::i2c;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAddress(u8),
    I2c(i2c::Error),
    SharedI2c(I2cDeviceError<i2c::Error>),
    Timeout,
}

impl From<i2c::Error> for Error {
    fn from(err: i2c::Error) -> Self {
        Self::I2c(err)
    }
}

impl From<I2cDeviceError<i2c::Error>> for Error {
    fn from(err: I2cDeviceError<i2c::Error>) -> Self {
        Self::SharedI2c(err)
    }
}
