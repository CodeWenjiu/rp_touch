pub(crate) const CMD_SW_RESET: u8 = 0x01;
pub(crate) const CMD_SLEEP_OUT: u8 = 0x11;
pub(crate) const CMD_DISPLAY_ON: u8 = 0x29;
pub(crate) const CMD_COLUMN_ADDR_SET: u8 = 0x2A;
pub(crate) const CMD_ROW_ADDR_SET: u8 = 0x2B;
pub(crate) const CMD_TEARING_EFFECT_OFF: u8 = 0x34;
pub(crate) const CMD_MEMORY_WRITE: u8 = 0x2C;
pub(crate) const CMD_COLOR_MODE: u8 = 0x3A;
pub(crate) const CMD_WRITE_BRIGHTNESS: u8 = 0x51;
pub(crate) const CMD_WRITE_CTRL_DISPLAY: u8 = 0x53;
pub(crate) const CMD_HIGH_CONTRAST_MODE: u8 = 0x58;
pub(crate) const CMD_WRHBMDISBV: u8 = 0x63;
pub(crate) const CMD_SPI_MODE: u8 = 0xC4;
pub(crate) const CMD_PAGE_SWITCH: u8 = 0xFE;

pub(crate) const DMA_CHUNK_BYTES: usize = 16_384;
pub(crate) const DEFAULT_ROWS_PER_BURST: usize = 64;

pub const MAX_STABLE_SCLK_HZ: u32 = 65_000_000;
pub(crate) const DEFAULT_SCLK_HZ: u32 = 65_000_000;
pub(crate) const BOARD_INIT_SCLK_HZ: u32 = 32_000_000;

pub(crate) const DEFAULT_CMD_PREFIX: u8 = 0x02;
pub(crate) const DEFAULT_DATA_PREFIX: u8 = 0x32;
pub(crate) const DEFAULT_WINDOW_X_OFFSET: u16 = 20;
pub(crate) const DEFAULT_WINDOW_Y_OFFSET: u16 = 0;

pub(crate) const BOARD_INIT_PAGE_PARAM: u8 = 0x00;
pub(crate) const BOARD_RESET_LOW_MS: u64 = 10;
pub(crate) const BOARD_RESET_SETTLE_MS: u64 = 120;
pub(crate) const BOARD_SLEEP_OUT_WAIT_MS: u64 = 120;
pub(crate) const BOARD_DISPLAY_ON_WAIT_MS: u64 = 70;
