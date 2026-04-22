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
            watermark_odr_samples: 8,
            size: FifoSize::Samples32,
            mode: FifoMode::Stream,
        }
    }
}
