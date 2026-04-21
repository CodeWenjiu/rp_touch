use core::fmt;

use embassy_rp::{gpio::Pull, i2c};

use crate::regs::QMI8658_I2C_ADDR;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    I2c(i2c::Error),
    InvalidAddress(u8),
    InvalidChipId(u8),
    Ctrl9Timeout,
}

impl From<i2c::Error> for Error {
    fn from(value: i2c::Error) -> Self {
        Self::I2c(value)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ImuRawSample {
    pub accel: [i16; 3],
    pub gyro: [i16; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ImuFrame {
    pub seq: u32,
    pub sample: ImuRawSample,
}

impl fmt::Display for ImuFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IMU,{},{},{},{},{},{},{}",
            self.seq,
            self.sample.accel[0],
            self.sample.accel[1],
            self.sample.accel[2],
            self.sample.gyro[0],
            self.sample.gyro[1],
            self.sample.gyro[2]
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuReport {
    Sample(ImuRawSample),
    ReadError(u32),
    InitError,
    InvalidChipId(u8),
    FifoConfigError,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Int1FifoStreamState {
    pub read_fail_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState {
    Starting,
    Running,
    InitFailed,
    InvalidChipId(u8),
    FifoConfigFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureStats {
    pub state: CaptureState,
    pub pushed_samples: u32,
    pub popped_samples: u32,
    pub dropped_samples: u32,
    pub read_fail_count: u32,
    pub latest_seq: Option<u32>,
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            state: CaptureState::Starting,
            pushed_samples: 0,
            popped_samples: 0,
            dropped_samples: 0,
            read_fail_count: 0,
            latest_seq: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FifoSize {
    Samples16,
    Samples32,
    Samples64,
    Samples128,
}

impl FifoSize {
    pub(crate) fn bits(self) -> u8 {
        match self {
            Self::Samples16 => 0b00,
            Self::Samples32 => 0b01,
            Self::Samples64 => 0b10,
            Self::Samples128 => 0b11,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FifoMode {
    Bypass,
    Fifo,
    Stream,
}

impl FifoMode {
    pub(crate) fn bits(self) -> u8 {
        match self {
            Self::Bypass => 0b00,
            Self::Fifo => 0b01,
            Self::Stream => 0b10,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FifoConfig {
    pub watermark_odr_samples: u8,
    pub size: FifoSize,
    pub mode: FifoMode,
}

impl Default for FifoConfig {
    fn default() -> Self {
        Self {
            watermark_odr_samples: 2,
            size: FifoSize::Samples32,
            mode: FifoMode::Stream,
        }
    }
}
