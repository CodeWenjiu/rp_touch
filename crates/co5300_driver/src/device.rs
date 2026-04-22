use embassy_rp::{
    Peri, bind_interrupts, dma,
    gpio::{Level, Output},
    peripherals,
    spi::{self, Spi},
};
use embassy_time::{Duration, Timer};

use crate::{
    config::*,
    framebuffer::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FrameBuffer},
    tuning::Co5300Tuning,
};

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
});

pub struct Co5300<'d> {
    spi: Spi<'d, peripherals::SPI1, spi::Async>,
    cs: Output<'d>,
    reset: Output<'d>,
    tuning: Co5300Tuning,
}

impl<'d> Co5300<'d> {
    pub fn new(
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

        Self {
            spi,
            cs,
            reset,
            tuning: Co5300Tuning::default(),
        }
    }

    pub fn set_tuning(&mut self, tuning: Co5300Tuning) {
        self.tuning = tuning;
    }

    pub fn tuning(&self) -> Co5300Tuning {
        self.tuning
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
        self.write_command(CMD_PAGE_SWITCH, &[BOARD_INIT_PAGE_PARAM])
            .await;
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
        let x0p = x0.saturating_add(self.tuning.x_offset);
        let x1p = x1.saturating_add(self.tuning.x_offset);
        let y0p = y0.saturating_add(self.tuning.y_offset);
        let y1p = y1.saturating_add(self.tuning.y_offset);

        let col = [(x0p >> 8) as u8, x0p as u8, (x1p >> 8) as u8, x1p as u8];
        let row = [(y0p >> 8) as u8, y0p as u8, (y1p >> 8) as u8, y1p as u8];

        self.write_command(CMD_COLUMN_ADDR_SET, &col).await;
        self.write_command(CMD_ROW_ADDR_SET, &row).await;
    }

    pub async fn write_framebuffer(&mut self, framebuffer: &FrameBuffer) {
        let row_bytes = DISPLAY_WIDTH * 2;
        let fb_bytes = framebuffer.as_bytes();
        let rows_per_burst = self.tuning.rows_per_burst.max(1);

        for y_start in (0..DISPLAY_HEIGHT).step_by(rows_per_burst) {
            let rows = core::cmp::min(rows_per_burst, DISPLAY_HEIGHT - y_start);
            let y_end = y_start + rows - 1;

            self.set_address_window(0, y_start as u16, (DISPLAY_WIDTH - 1) as u16, y_end as u16)
                .await;

            let start = y_start * row_bytes;
            let end = start + rows * row_bytes;

            self.cs.set_low();
            let cmd_header = Self::qspi_flash_header(self.tuning.data_prefix, CMD_MEMORY_WRITE);
            self.push_raw(&cmd_header).await;
            self.push_raw(&fb_bytes[start..end]).await;
            self.cs.set_high();
        }
    }

    pub async fn refresh_loop(&mut self, framebuffer: &FrameBuffer, period: Duration) -> ! {
        loop {
            self.write_framebuffer(framebuffer).await;
            Timer::after(period).await;
        }
    }

    pub async fn write_command(&mut self, command: u8, params: &[u8]) {
        self.cs.set_low();
        let cmd_header = Self::qspi_flash_header(self.tuning.cmd_prefix, command);
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

impl Co5300<'static> {
    pub fn new_default() -> Self {
        unsafe {
            Self::new(
                peripherals::SPI1::steal(),
                peripherals::DMA_CH0::steal(),
                peripherals::PIN_9::steal(),
                peripherals::PIN_10::steal(),
                peripherals::PIN_11::steal(),
                peripherals::PIN_15::steal(),
            )
        }
    }
}
