use crate::regs::FT3168_I2C_ADDR;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ft3168Config {
    pub address: u8,
    pub i2c_frequency_hz: u32,
}

impl Default for Ft3168Config {
    fn default() -> Self {
        Self {
            address: FT3168_I2C_ADDR,
            i2c_frequency_hz: 400_000,
        }
    }
}
