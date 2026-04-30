use i2c_bus::BusError;

/// FT3168 driver error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAddress(u8),
    InvalidChipId(u8),
    Bus(BusError),
}

impl From<BusError> for Error {
    fn from(e: BusError) -> Self {
        Self::Bus(e)
    }
}
