use embassy_rp::{
    Peri, bind_interrupts,
    clocks::clk_sys_freq,
    dma,
    gpio::{Drive, Level, Output, SlewRate},
    peripherals,
    pio::{self, Direction, FifoJoin, Pio, ShiftDirection, StateMachine},
    pio_programs::clock_divider::calculate_pio_clock_divider,
};
use embassy_time::{Duration, Timer};

use crate::{
    config::*,
    framebuffer::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FrameBuffer},
    tuning::Co5300Tuning,
};

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TxMode {
    Single,
    Quad,
}

struct PioTx<'d> {
    sm: StateMachine<'d, peripherals::PIO0, 0>,
    dma: dma::Channel<'d>,
    cfg_single: pio::Config<'d, peripherals::PIO0>,
    cfg_quad: pio::Config<'d, peripherals::PIO0>,
    mode: TxMode,
    quad_dma_active: bool,
}

impl<'d> PioTx<'d> {
    fn effective_serial_hz(requested_hz: u32) -> u32 {
        let capped_by_board = requested_hz.max(1).min(MAX_STABLE_SCLK_HZ);
        let capped_by_pio = clk_sys_freq().saturating_div(2).max(1);
        capped_by_board.min(capped_by_pio)
    }

    fn new(
        serial_hz: u32,
        pio: Peri<'d, peripherals::PIO0>,
        dma_ch: Peri<'d, peripherals::DMA_CH0>,
        clk: Peri<'d, peripherals::PIN_10>,
        sio0: Peri<'d, peripherals::PIN_11>,
        sio1: Peri<'d, peripherals::PIN_12>,
        sio2: Peri<'d, peripherals::PIN_13>,
        sio3: Peri<'d, peripherals::PIN_14>,
    ) -> Self {
        let serial_hz = Self::effective_serial_hz(serial_hz);
        let mut pio = Pio::new(pio, Irqs);
        let dma = dma::Channel::new(dma_ch, Irqs);

        let mut clk_pin = pio.common.make_pio_pin(clk);
        let mut sio0_pin = pio.common.make_pio_pin(sio0);
        let mut sio1_pin = pio.common.make_pio_pin(sio1);
        let mut sio2_pin = pio.common.make_pio_pin(sio2);
        let mut sio3_pin = pio.common.make_pio_pin(sio3);

        for pin in [
            &mut clk_pin,
            &mut sio0_pin,
            &mut sio1_pin,
            &mut sio2_pin,
            &mut sio3_pin,
        ] {
            pin.set_drive_strength(Drive::_12mA);
            pin.set_slew_rate(SlewRate::Fast);
        }

        let single_program = pio::program::pio_asm!(
            ".side_set 1",
            ".wrap_target",
            "out pins, 1 side 0",
            "nop side 1",
            ".wrap",
        );
        let quad_program = pio::program::pio_asm!(
            ".side_set 1",
            ".wrap_target",
            "out pins, 4 side 0",
            "nop side 1",
            ".wrap",
        );
        let single_program = pio.common.load_program(&single_program.program);
        let quad_program = pio.common.load_program(&quad_program.program);

        let mut cfg_single = pio::Config::default();
        cfg_single.use_program(&single_program, &[&clk_pin]);
        cfg_single.set_out_pins(&[&sio0_pin]);
        cfg_single.shift_out.auto_fill = true;
        cfg_single.shift_out.direction = ShiftDirection::Left;
        cfg_single.shift_out.threshold = 8;
        cfg_single.fifo_join = FifoJoin::TxOnly;

        let mut cfg_quad = pio::Config::default();
        cfg_quad.use_program(&quad_program, &[&clk_pin]);
        cfg_quad.set_out_pins(&[&sio0_pin, &sio1_pin, &sio2_pin, &sio3_pin]);
        cfg_quad.shift_out.auto_fill = true;
        cfg_quad.shift_out.direction = ShiftDirection::Left;
        cfg_quad.shift_out.threshold = 8;
        cfg_quad.fifo_join = FifoJoin::TxOnly;

        let divider = calculate_pio_clock_divider(serial_hz.saturating_mul(2));
        cfg_single.clock_divider = divider;
        cfg_quad.clock_divider = divider;

        let mut sm = pio.sm0;
        sm.set_config(&cfg_single);
        sm.set_pins(
            Level::Low,
            &[&clk_pin, &sio0_pin, &sio1_pin, &sio2_pin, &sio3_pin],
        );
        sm.set_pin_dirs(
            Direction::Out,
            &[&clk_pin, &sio0_pin, &sio1_pin, &sio2_pin, &sio3_pin],
        );
        sm.clear_fifos();
        sm.set_enable(true);

        Self {
            sm,
            dma,
            cfg_single,
            cfg_quad,
            mode: TxMode::Single,
            quad_dma_active: false,
        }
    }

    async fn write_single(&mut self, bytes: &[u8]) {
        self.write(TxMode::Single, bytes).await;
    }

    async fn write_quad(&mut self, bytes: &[u8]) {
        self.write(TxMode::Quad, bytes).await;
    }

    async fn write(&mut self, mode: TxMode, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        self.switch_mode(mode);

        for chunk in bytes.chunks(DMA_CHUNK_BYTES) {
            self.sm.tx().dma_push(&mut self.dma, chunk, false).await;
        }

        self.flush_tx();
    }

    fn write_blocking(&mut self, mode: TxMode, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        self.switch_mode(mode);
        self.start_dma_bytes(bytes);
        self.wait_dma_and_tx_done();
    }

    fn switch_mode(&mut self, mode: TxMode) {
        self.wait_dma_and_tx_done();

        if self.mode == mode {
            return;
        }

        self.sm.set_enable(false);
        match mode {
            TxMode::Single => self.sm.set_config(&self.cfg_single),
            TxMode::Quad => self.sm.set_config(&self.cfg_quad),
        }
        self.sm.clear_fifos();
        self.sm.restart();
        self.sm.set_enable(true);
        self.mode = mode;
    }

    fn set_serial_hz(&mut self, serial_hz: u32) {
        let serial_hz = Self::effective_serial_hz(serial_hz);
        let divider = calculate_pio_clock_divider(serial_hz.saturating_mul(2));
        self.cfg_single.clock_divider = divider;
        self.cfg_quad.clock_divider = divider;

        self.wait_dma_and_tx_done();
        self.sm.set_enable(false);
        match self.mode {
            TxMode::Single => self.sm.set_config(&self.cfg_single),
            TxMode::Quad => self.sm.set_config(&self.cfg_quad),
        }
        self.sm.clear_fifos();
        self.sm.restart();
        self.sm.set_enable(true);
    }

    fn flush_tx(&mut self) {
        while !self.sm.tx().empty() {}
        while !self.sm.tx().stalled() {}
    }

    fn start_quad_dma(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        self.switch_mode(TxMode::Quad);
        self.start_dma_bytes(bytes);
        self.quad_dma_active = true;
    }

    fn poll_quad_dma_complete(&mut self) -> bool {
        if !self.quad_dma_active {
            return true;
        }
        if self.dma_busy() {
            return false;
        }

        self.flush_tx();
        self.quad_dma_active = false;
        true
    }

    fn wait_dma_and_tx_done(&mut self) {
        while self.dma_busy() {}
        self.flush_tx();
    }

    fn dma_busy(&self) -> bool {
        self.dma.regs().ctrl_trig().read().busy()
    }

    fn start_dma_bytes(&mut self, bytes: &[u8]) {
        unsafe {
            // Keep transfer alive in hardware; completion is polled via DMA busy flag.
            let transfer = self.dma.write(
                bytes,
                self.sm.tx_fifo_ptr() as *mut u8,
                self.sm.tx_treq(),
                false,
            );
            core::mem::forget(transfer);
        }
    }
}

pub struct Co5300<'d> {
    tx: PioTx<'d>,
    cs: Output<'d>,
    reset: Output<'d>,
    tuning: Co5300Tuning,
    stripe_transfer_active: bool,
}

impl<'d> Co5300<'d> {
    pub fn new(
        pio: Peri<'d, peripherals::PIO0>,
        dma_ch: Peri<'d, peripherals::DMA_CH0>,
        cs: Peri<'d, peripherals::PIN_9>,
        clk: Peri<'d, peripherals::PIN_10>,
        sio0: Peri<'d, peripherals::PIN_11>,
        sio1: Peri<'d, peripherals::PIN_12>,
        sio2: Peri<'d, peripherals::PIN_13>,
        sio3: Peri<'d, peripherals::PIN_14>,
        reset: Peri<'d, peripherals::PIN_15>,
    ) -> Self {
        let tuning = Co5300Tuning::default();
        let tx = PioTx::new(tuning.sclk_hz, pio, dma_ch, clk, sio0, sio1, sio2, sio3);

        let mut cs = Output::new(cs, Level::High);
        cs.set_high();
        let mut reset = Output::new(reset, Level::High);
        reset.set_high();

        Self {
            tx,
            cs,
            reset,
            tuning,
            stripe_transfer_active: false,
        }
    }

    pub fn set_tuning(&mut self, tuning: Co5300Tuning) {
        self.tx.set_serial_hz(tuning.sclk_hz);
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
        // Keep command-phase timing conservative, then switch back to the runtime bus rate.
        self.tx.set_serial_hz(BOARD_INIT_SCLK_HZ);
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
        self.tx.set_serial_hz(self.tuning.sclk_hz);
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
            self.push_single(&cmd_header).await;
            self.push_quad(&fb_bytes[start..end]).await;
            self.cs.set_high();
        }
    }

    pub async fn write_framebuffer_region(
        &mut self,
        framebuffer: &FrameBuffer,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        if width == 0 || height == 0 || x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
            return;
        }

        let x0 = x;
        let y0 = y;
        let x1 = core::cmp::min(x.saturating_add(width), DISPLAY_WIDTH) - 1;
        let y1 = core::cmp::min(y.saturating_add(height), DISPLAY_HEIGHT) - 1;

        let full_row_bytes = DISPLAY_WIDTH * 2;
        let region_row_bytes = (x1 - x0 + 1) * 2;
        let fb_bytes = framebuffer.as_bytes();

        for row in y0..=y1 {
            self.set_address_window(x0 as u16, row as u16, x1 as u16, row as u16)
                .await;

            let start = row * full_row_bytes + x0 * 2;
            let end = start + region_row_bytes;

            self.cs.set_low();
            let cmd_header = Self::qspi_flash_header(self.tuning.data_prefix, CMD_MEMORY_WRITE);
            self.push_single(&cmd_header).await;
            self.push_quad(&fb_bytes[start..end]).await;
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
        self.push_single(&cmd_header).await;

        if !params.is_empty() {
            self.push_single(params).await;
        }

        self.cs.set_high();
    }

    pub async fn write_data(&mut self, payload: &[u8]) {
        self.cs.set_low();
        self.push_single(payload).await;
        self.cs.set_high();
    }

    pub fn set_address_window_blocking(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let x0p = x0.saturating_add(self.tuning.x_offset);
        let x1p = x1.saturating_add(self.tuning.x_offset);
        let y0p = y0.saturating_add(self.tuning.y_offset);
        let y1p = y1.saturating_add(self.tuning.y_offset);

        let col = [(x0p >> 8) as u8, x0p as u8, (x1p >> 8) as u8, x1p as u8];
        let row = [(y0p >> 8) as u8, y0p as u8, (y1p >> 8) as u8, y1p as u8];

        self.write_command_blocking(CMD_COLUMN_ADDR_SET, &col);
        self.write_command_blocking(CMD_ROW_ADDR_SET, &row);
    }

    pub fn write_command_blocking(&mut self, command: u8, params: &[u8]) {
        self.cs.set_low();
        let cmd_header = Self::qspi_flash_header(self.tuning.cmd_prefix, command);
        self.push_single_blocking(&cmd_header);

        if !params.is_empty() {
            self.push_single_blocking(params);
        }

        self.cs.set_high();
    }

    pub fn begin_fullwidth_stripe_transfer(&mut self, y_start: usize, rows: usize, pixels: &[u16]) {
        if rows == 0 || y_start >= DISPLAY_HEIGHT || self.stripe_transfer_active {
            return;
        }

        let clamped_rows = core::cmp::min(rows, DISPLAY_HEIGHT - y_start);
        let expected_words = DISPLAY_WIDTH * clamped_rows;
        if pixels.len() < expected_words {
            return;
        }

        self.set_address_window_blocking(
            0,
            y_start as u16,
            (DISPLAY_WIDTH - 1) as u16,
            (y_start + clamped_rows - 1) as u16,
        );

        self.cs.set_low();
        let cmd_header = Self::qspi_flash_header(self.tuning.data_prefix, CMD_MEMORY_WRITE);
        self.push_single_blocking(&cmd_header);

        let payload = unsafe {
            core::slice::from_raw_parts(
                pixels.as_ptr() as *const u8,
                expected_words * core::mem::size_of::<u16>(),
            )
        };
        self.push_quad_dma_start(payload);
        self.stripe_transfer_active = true;
    }

    pub fn poll_fullwidth_stripe_transfer_done(&mut self) -> bool {
        if !self.stripe_transfer_active {
            return true;
        }

        if !self.tx.poll_quad_dma_complete() {
            return false;
        }

        self.cs.set_high();
        self.stripe_transfer_active = false;
        true
    }

    pub fn wait_fullwidth_stripe_transfer_done(&mut self) {
        while !self.poll_fullwidth_stripe_transfer_done() {}
    }

    #[inline]
    fn qspi_flash_header(opcode: u8, reg: u8) -> [u8; 4] {
        [opcode, 0x00, reg, 0x00]
    }

    async fn push_single(&mut self, bytes: &[u8]) {
        self.tx.write_single(bytes).await;
    }

    async fn push_quad(&mut self, bytes: &[u8]) {
        self.tx.write_quad(bytes).await;
    }

    fn push_single_blocking(&mut self, bytes: &[u8]) {
        self.tx.write_blocking(TxMode::Single, bytes);
    }

    fn push_quad_dma_start(&mut self, bytes: &[u8]) {
        self.tx.start_quad_dma(bytes);
    }
}

impl Co5300<'static> {
    pub fn new_default() -> Self {
        unsafe {
            Self::new(
                peripherals::PIO0::steal(),
                peripherals::DMA_CH0::steal(),
                peripherals::PIN_9::steal(),
                peripherals::PIN_10::steal(),
                peripherals::PIN_11::steal(),
                peripherals::PIN_12::steal(),
                peripherals::PIN_13::steal(),
                peripherals::PIN_14::steal(),
                peripherals::PIN_15::steal(),
            )
        }
    }
}
