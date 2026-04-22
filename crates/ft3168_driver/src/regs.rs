pub const FT3168_I2C_ADDR: u8 = 0x38;

pub const FT3168_REG_TOUCH_STATUS: u8 = 0x02;
pub const FT3168_REG_TOUCH_DATA_START: u8 = 0x03;
pub const FT3168_REG_CHIP_ID: u8 = 0xA3;
pub const FT3168_REG_FIRMWARE_ID: u8 = 0xA6;

pub const FT3168_MAX_TOUCH_POINTS: usize = 5;
pub const FT3168_TOUCH_POINT_BYTES: usize = 6;

pub(crate) const FT3168_BOOT_WAIT_MS: u64 = 15;
pub(crate) const FT3168_RETRY_COUNT: usize = 5;
