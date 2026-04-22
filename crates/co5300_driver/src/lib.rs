#![no_std]

use embassy_rp::{
    Peri, bind_interrupts, dma,
    gpio::{Level, Output},
    peripherals,
    spi::{self, Spi},
};
use embassy_time::{Duration, Timer};

pub const DISPLAY_WIDTH: usize = 280;
pub const DISPLAY_HEIGHT: usize = 456;
pub const PIXEL_COUNT: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT;
pub const FRAMEBUFFER_BYTES: usize = PIXEL_COUNT * 2;

const CMD_SW_RESET: u8 = 0x01;
const CMD_SLEEP_OUT: u8 = 0x11;
const CMD_DISPLAY_ON: u8 = 0x29;
const CMD_COLUMN_ADDR_SET: u8 = 0x2A;
const CMD_ROW_ADDR_SET: u8 = 0x2B;
const CMD_TEARING_EFFECT_OFF: u8 = 0x34;
const CMD_MEMORY_WRITE: u8 = 0x2C;
const CMD_COLOR_MODE: u8 = 0x3A;
const CMD_WRITE_BRIGHTNESS: u8 = 0x51;
const CMD_WRITE_CTRL_DISPLAY: u8 = 0x53;
const CMD_HIGH_CONTRAST_MODE: u8 = 0x58;
const CMD_WRHBMDISBV: u8 = 0x63;
const CMD_SPI_MODE: u8 = 0xC4;
const CMD_PAGE_SWITCH: u8 = 0xFE;

const DMA_CHUNK_BYTES: usize = 4096;
pub const MAX_STABLE_SCLK_HZ: u32 = 32_000_000;
const BOARD_SCLK_HZ: u32 = MAX_STABLE_SCLK_HZ;
const BOARD_CMD_PREFIX: u8 = 0x02;
const BOARD_DATA_PREFIX: u8 = 0x02;
const BOARD_INIT_PAGE_PARAM: u8 = 0x00;
const BOARD_RESET_LOW_MS: u64 = 10;
const BOARD_RESET_SETTLE_MS: u64 = 120;
const BOARD_SLEEP_OUT_WAIT_MS: u64 = 120;
const BOARD_DISPLAY_ON_WAIT_MS: u64 = 70;

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
});

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawError {
    OutOfBounds { x: usize, y: usize },
}

#[derive(Clone)]
pub struct FrameBuffer {
    pixels: [u16; PIXEL_COUNT],
}

impl FrameBuffer {
    pub const fn new() -> Self {
        Self {
            pixels: [0; PIXEL_COUNT],
        }
    }

    pub fn fill_rgb565(&mut self, color: u16) {
        self.pixels.fill(color.to_be());
    }

    pub fn set_pixel_rgb565(&mut self, x: usize, y: usize, color: u16) -> Result<(), DrawError> {
        if x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
            return Err(DrawError::OutOfBounds { x, y });
        }

        let idx = y * DISPLAY_WIDTH + x;
        self.pixels[idx] = color.to_be();
        Ok(())
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16] {
        &mut self.pixels
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, FRAMEBUFFER_BYTES) }
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Co5300<'d> {
    spi: Spi<'d, peripherals::SPI1, spi::Async>,
    cs: Output<'d>,
    reset: Output<'d>,
}

impl<'d> Co5300<'d> {
    pub fn new_default(
        spi: Peri<'d, peripherals::SPI1>,
        dma_ch: Peri<'d, peripherals::DMA_CH0>,
        cs: Peri<'d, peripherals::PIN_9>,
        clk: Peri<'d, peripherals::PIN_10>,
        mosi: Peri<'d, peripherals::PIN_11>,
        reset: Peri<'d, peripherals::PIN_15>,
    ) -> Self {
        let mut spi_cfg = spi::Config::default();
        spi_cfg.frequency = BOARD_SCLK_HZ.min(MAX_STABLE_SCLK_HZ);
        let spi = Spi::new_txonly(spi, clk, mosi, dma_ch, Irqs, spi_cfg);

        let mut cs = Output::new(cs, Level::High);
        cs.set_high();
        let mut reset = Output::new(reset, Level::High);
        reset.set_high();

        Self { spi, cs, reset }
    }

    pub async fn hard_reset(&mut self) {
        self.reset.set_low();
        Timer::after(Duration::from_millis(BOARD_RESET_LOW_MS)).await;
        self.reset.set_high();
        Timer::after(Duration::from_millis(BOARD_RESET_SETTLE_MS)).await;
    }

    pub async fn init_default(&mut self) {
        self.hard_reset().await;
        self.write_command(CMD_SW_RESET, &[]).await;
        Timer::after(Duration::from_millis(10)).await;

        self.write_command(CMD_SLEEP_OUT, &[]).await;
        Timer::after(Duration::from_millis(BOARD_SLEEP_OUT_WAIT_MS)).await;

        self.write_command(CMD_TEARING_EFFECT_OFF, &[]).await;
        self.write_command(CMD_PAGE_SWITCH, &[BOARD_INIT_PAGE_PARAM]).await;
        self.write_command(CMD_SPI_MODE, &[0x80]).await;
        self.write_command(CMD_COLOR_MODE, &[0x55]).await;
        self.write_command(CMD_WRITE_CTRL_DISPLAY, &[0x20]).await;
        self.write_command(CMD_WRHBMDISBV, &[0xFF]).await;

        self.write_command(CMD_DISPLAY_ON, &[]).await;
        Timer::after(Duration::from_millis(BOARD_DISPLAY_ON_WAIT_MS)).await;
        self.write_command(CMD_WRITE_BRIGHTNESS, &[0xFF]).await;
        self.write_command(CMD_HIGH_CONTRAST_MODE, &[0x00]).await;
    }

    pub async fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let col = [(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8];
        let row = [(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8];

        self.write_command(CMD_COLUMN_ADDR_SET, &col).await;
        self.write_command(CMD_ROW_ADDR_SET, &row).await;
    }

    pub async fn write_framebuffer(&mut self, framebuffer: &FrameBuffer) {
        self.set_address_window(
            0,
            0,
            (DISPLAY_WIDTH - 1) as u16,
            (DISPLAY_HEIGHT - 1) as u16,
        )
        .await;
        self.cs.set_low();
        let cmd_header = Self::qspi_flash_header(BOARD_DATA_PREFIX, CMD_MEMORY_WRITE);
        self.push_raw(&cmd_header).await;
        self.push_raw(framebuffer.as_bytes()).await;
        self.cs.set_high();
    }

    pub async fn refresh_loop(&mut self, framebuffer: &FrameBuffer, period: Duration) -> ! {
        loop {
            self.write_framebuffer(framebuffer).await;
            Timer::after(period).await;
        }
    }

    pub async fn write_command(&mut self, command: u8, params: &[u8]) {
        self.cs.set_low();
        let cmd_header = Self::qspi_flash_header(BOARD_CMD_PREFIX, command);
        self.push_raw(&cmd_header).await;

        if !params.is_empty() {
            self.push_raw(params).await;
        }

        self.cs.set_high();
    }

    pub async fn write_data(&mut self, payload: &[u8]) {
        self.cs.set_low();
        self.push_raw(payload).await;
        self.cs.set_high();
    }

    #[inline]
    fn qspi_flash_header(opcode: u8, reg: u8) -> [u8; 4] {
        [opcode, 0x00, reg, 0x00]
    }

    async fn push_raw(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        for chunk in bytes.chunks(DMA_CHUNK_BYTES) {
            let _ = self.spi.write(chunk).await;
        }
    }
}
