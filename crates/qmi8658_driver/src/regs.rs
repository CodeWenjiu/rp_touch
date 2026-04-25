pub const QMI8658_I2C_ADDR: u8 = 0x6B;
pub const QMI8658_CHIP_ID: u8 = 0x05;
pub const QMI8658_REG_WHO_AM_I: u8 = 0x00;
pub const QMI8658_REG_CTRL1: u8 = 0x02;
pub const QMI8658_REG_CTRL2: u8 = 0x03;
pub const QMI8658_REG_CTRL3: u8 = 0x04;
pub const QMI8658_REG_CTRL7: u8 = 0x08;
pub const QMI8658_REG_CTRL8: u8 = 0x09;
pub const QMI8658_REG_CTRL9: u8 = 0x0A;
pub const QMI8658_REG_FIFO_WTM_TH: u8 = 0x13;
pub const QMI8658_REG_FIFO_CTRL: u8 = 0x14;
pub const QMI8658_REG_FIFO_SMPL_CNT: u8 = 0x15;
pub const QMI8658_REG_FIFO_STATUS: u8 = 0x16;
pub const QMI8658_REG_FIFO_DATA: u8 = 0x17;
pub const QMI8658_REG_STATUSINT: u8 = 0x2D;
pub const QMI8658_REG_TEMP_L: u8 = 0x33;
pub const QMI8658_REG_AX_L: u8 = 0x35;
pub const QMI8658_REG_RESET: u8 = 0x60;

pub(crate) const CTRL1_BASE: u8 = 0x60;
pub(crate) const CTRL1_ADDR_AI: u8 = 1 << 6;
pub(crate) const CTRL1_FIFO_INT_SEL: u8 = 1 << 2;
pub(crate) const CTRL1_INT1_EN: u8 = 1 << 3;

pub(crate) const CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT: u8 = 1 << 7;

pub(crate) const CTRL7_ACCEL_ENABLE: u8 = 1 << 0;
pub(crate) const CTRL7_GYRO_ENABLE: u8 = 1 << 1;

// CTRL2/3 values for a typical 6-DoF bring-up:
// Accel  +/-8g, ODR setting 0b0011 (nominal 1000Hz accel-only, 896.8Hz in 6-DoF)
// Gyro   +/-512dps, ODR setting 0b0011 (896.8Hz)
pub(crate) const CTRL2_DEFAULT_ACCEL_8G_1000HZ: u8 = 0x23;
pub(crate) const CTRL3_DEFAULT_GYRO_512DPS_1000HZ: u8 = 0x53;

pub(crate) const CTRL9_CMD_ACK: u8 = 0x00;
pub(crate) const CTRL9_CMD_RST_FIFO: u8 = 0x04;
pub(crate) const CTRL9_CMD_REQ_FIFO: u8 = 0x05;

pub(crate) const FIFO_CTRL_RD_MODE: u8 = 1 << 7;
pub(crate) const FIFO_CTRL_SIZE_SHIFT: u8 = 2;
pub(crate) const FIFO_CTRL_MODE_MASK: u8 = 0b11;

pub(crate) const STATUSINT_CMD_DONE: u8 = 1 << 7;
pub(crate) const RESET_SOFT_CMD: u8 = 0xB0;
