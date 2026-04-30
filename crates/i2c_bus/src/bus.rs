use core::sync::atomic::Ordering;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{
    i2c::{self, Async, I2c as RpI2c, Instance},
    interrupt::typelevel::Binding,
    pac,
    peripherals,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{block_for, Duration, Timer, with_timeout};
use embedded_hal_async::i2c::I2c;
use static_cell::StaticCell;

use crate::device::DeviceIo;
use crate::error::BusError;
use crate::types::{BusConfig, BusStats};

// ── RetryingDevice ──────────────────────────────────────────────────────────

/// A device handle with per-operation timeout, retry loop, and bus statistics.
pub struct RetryingDevice<'d, BUS> {
    inner: I2cDevice<'d, CriticalSectionRawMutex, BUS>,
    stats: &'d BusStats,
    timeout_ms: u64,
    max_retries: u8,
}

impl<'d, BUS> RetryingDevice<'d, BUS>
where
    BUS: I2c<Error = i2c::Error>,
{
    /// Create a new retrying device handle.
    pub fn new(
        inner: I2cDevice<'d, CriticalSectionRawMutex, BUS>,
        stats: &'d BusStats,
        timeout_ms: u64,
        max_retries: u8,
    ) -> Self {
        Self {
            inner,
            stats,
            timeout_ms,
            max_retries,
        }
    }

    /// Increment `total_ops` at the start of each operation.
    fn begin_op(&self) {
        self.stats.total_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful attempt.
    fn record_success(&self, attempts: u8) {
        self.stats
            .successful_ops
            .fetch_add(1, Ordering::Relaxed);
        self.stats.consecutive_errors.store(0, Ordering::Relaxed);
        if attempts > 0 {
            self.stats.retried_ops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a failure (non-retryable or exhausted).
    fn record_failure(&self, err: &BusError, attempts: u8) {
        self.stats.error_ops.fetch_add(1, Ordering::Relaxed);
        self.stats
            .consecutive_errors
            .fetch_add(1, Ordering::Relaxed);
        if attempts > 0 {
            self.stats.retried_ops.fetch_add(1, Ordering::Relaxed);
        }
        if matches!(err, BusError::Timeout) {
            self.stats
                .timed_out_ops
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

// Macro: retry an I2C operation with timeout + back-off.
//
// `$dev` is the I2cDevice (accessible as `self.inner`),
// `$op` is the async expression returning `Result<R, E>` where `E: Into<BusError>`.
macro_rules! i2c_retry {
    ($self:expr, $op:expr) => {{
        $self.begin_op();

        let mut __attempt: u8 = 0;
        let mut __last_err: Option<BusError> = None;

        loop {
            let __result = with_timeout(
                Duration::from_millis($self.timeout_ms),
                $op,
            )
            .await;

            match __result {
                Ok(Ok(__val)) => {
                    $self.record_success(__attempt);
                    break Ok(__val);
                }
                Ok(Err(__e)) => {
                    __last_err = Some(__e.into());
                }
                Err(_timeout) => {
                    __last_err = Some(BusError::Timeout);
                }
            }

            let __err = __last_err
                .take()
                .expect("last_err set after non-success");

            if __err.is_retryable() && __attempt < $self.max_retries {
                __attempt += 1;
                Timer::after(Duration::from_micros(200)).await;
            } else {
                $self.record_failure(&__err, __attempt);
                break Err(if __err.is_retryable() {
                    BusError::Fatal(match __err {
                        BusError::I2c(e) => e,
                        _ => i2c::Error::Abort(i2c::AbortReason::NoAcknowledge),
                    })
                } else {
                    __err
                });
            }
        }
    }};
}

impl<'d, BUS> DeviceIo for RetryingDevice<'d, BUS>
where
    BUS: I2c<Error = i2c::Error>,
{
    async fn write_reg(
        &mut self,
        address: u8,
        reg: u8,
        value: u8,
    ) -> Result<(), BusError> {
        let buf = [reg, value];
        i2c_retry!(self, self.inner.write(address, &buf))
    }

    async fn read_reg(&mut self, address: u8, reg: u8) -> Result<u8, BusError> {
        let mut out = [0u8; 1];
        i2c_retry!(self, self.inner.write_read(address, &[reg], &mut out))?;
        Ok(out[0])
    }

    async fn read_regs(
        &mut self,
        address: u8,
        start_reg: u8,
        buf: &mut [u8],
    ) -> Result<(), BusError> {
        i2c_retry!(self, self.inner.write_read(address, &[start_reg], buf))
    }

    async fn write_read(
        &mut self,
        address: u8,
        write_buf: &[u8],
        read_buf: &mut [u8],
    ) -> Result<(), BusError> {
        i2c_retry!(self, self.inner.write_read(address, write_buf, read_buf))
    }
}

// ── SharedBus ───────────────────────────────────────────────────────────────

/// Type alias for the canonical RP235x I2C1 shared bus.
pub type SharedI2c1Bus = SharedBus<'static, peripherals::I2C1>;

/// Owns the shared I2C bus, statistics, and recovery logic.
pub struct SharedBus<'d, PERI: Instance> {
    mutex: &'d Mutex<CriticalSectionRawMutex, RpI2c<'d, PERI, Async>>,
    stats: &'d BusStats,
    config: BusConfig,
}

impl SharedBus<'static, peripherals::I2C1> {
    /// Initialise the shared bus singleton.
    pub fn init(
        cell: &'static StaticCell<Self>,
        stats_cell: &'static StaticCell<BusStats>,
        i2c: RpI2c<'static, peripherals::I2C1, Async>,
        config: BusConfig,
    ) -> &'static Self {
        static BUS_MUTEX: StaticCell<
            Mutex<CriticalSectionRawMutex, RpI2c<'static, peripherals::I2C1, Async>>,
        > = StaticCell::new();
        let stats = stats_cell.init(BusStats::new());
        let mutex = BUS_MUTEX.init(Mutex::new(i2c));
        cell.init(Self {
            mutex,
            stats,
            config,
        })
    }

    /// Create a retrying device handle.
    pub fn device(
        &'static self,
        timeout_ms: u64,
        max_retries: u8,
    ) -> RetryingDevice<'static, RpI2c<'static, peripherals::I2C1, Async>> {
        RetryingDevice::new(
            I2cDevice::new(self.mutex),
            self.stats,
            timeout_ms,
            max_retries,
        )
    }

    /// Bus statistics reference.
    pub fn stats(&self) -> &BusStats {
        self.stats
    }

    /// Recover a hung I2C bus.
    pub async fn recover(
        &self,
        irqs: impl Binding<
            <peripherals::I2C1 as Instance>::Interrupt,
            i2c::InterruptHandler<peripherals::I2C1>,
        >,
    ) {
        let mut guard = self.mutex.lock().await;
        self.reset_peripheral();
        self.pulse_i2c_lines();
        self.reset_peripheral();
        let new_i2c = self.reinit_i2c(irqs);
        *guard = new_i2c;
        self.stats.recovery_count.fetch_add(1, Ordering::Relaxed);
    }

    // ── private helpers ──────────────────────────────────────────────────

    fn reset_peripheral(&self) {
        let resets = pac::RESETS;
        resets.reset().modify(|w| w.set_i2c1(true));
        block_for(Duration::from_micros(self.config.reset_delay_us as u64));
        resets.reset().modify(|w| w.set_i2c1(false));
        while !resets.reset_done().read().i2c1() {}
    }

    /// Bit-bang SCL pulses + STOP via PAC (no `AnyPin::steal`).
    #[allow(unused_unsafe)]
    fn pulse_i2c_lines(&self) {
        let scl = self.config.scl_pin as usize;
        let sda = self.config.sda_pin as usize;

        // SAFETY: I2C1 is in reset.  We temporarily reconfigure SCL/SDA
        // to SIO for the pulse and restore to I2C before returning.
        unsafe {
            // Switch to SIO mode.
            pac::IO_BANK0
                .gpio(scl)
                .ctrl()
                .modify(|w| w.set_funcsel(5));
            pac::IO_BANK0
                .gpio(sda)
                .ctrl()
                .modify(|w| w.set_funcsel(5));

            // Enable pull-ups.
            pac::PADS_BANK0.gpio(scl).modify(|w| w.set_pue(true));
            pac::PADS_BANK0.gpio(sda).modify(|w| w.set_pue(true));

            // Release both lines.
            pac::SIO.gpio_oe(scl).value_clr().write(|w| *w = 1);
            pac::SIO.gpio_oe(sda).value_clr().write(|w| *w = 1);
            pac::SIO.gpio_out(scl).value_clr().write(|w| *w = 1);
            pac::SIO.gpio_out(sda).value_clr().write(|w| *w = 1);

            let delay =
                || block_for(Duration::from_micros(self.config.bus_delay_us as u64));
            delay();

            // 9 SCL pulses.
            for _ in 0..self.config.pulse_count {
                pac::SIO.gpio_out(scl).value_clr().write(|w| *w = 1);
                pac::SIO.gpio_oe(scl).value_set().write(|w| *w = 1);
                delay();
                pac::SIO.gpio_oe(scl).value_clr().write(|w| *w = 1);
                delay();
            }

            // STOP condition.
            pac::SIO.gpio_out(sda).value_clr().write(|w| *w = 1);
            pac::SIO.gpio_oe(sda).value_set().write(|w| *w = 1);
            delay();
            pac::SIO.gpio_oe(scl).value_clr().write(|w| *w = 1);
            delay();
            pac::SIO.gpio_oe(sda).value_clr().write(|w| *w = 1);
            delay();

            // Restore I2C function.
            pac::IO_BANK0
                .gpio(scl)
                .ctrl()
                .modify(|w| w.set_funcsel(1));
            pac::IO_BANK0
                .gpio(sda)
                .ctrl()
                .modify(|w| w.set_funcsel(1));
        }
    }

    /// Re-create the `I2c` instance after hardware reset.
    ///
    /// # Safety
    ///
    /// Only place where `unsafe` steal occurs in I2C.  Safe because:
    /// - Bus mutex is held (exclusive access).
    /// - Old I2c is dropped when `*guard = new_i2c`.
    /// - Hardware was reset beforehand.
    #[allow(unsafe_code)]
    fn reinit_i2c(
        &self,
        irqs: impl Binding<
            <peripherals::I2C1 as Instance>::Interrupt,
            i2c::InterruptHandler<peripherals::I2C1>,
        >,
    ) -> RpI2c<'static, peripherals::I2C1, Async> {
        // SAFETY: see doc comment above.
        unsafe {
            let peri = peripherals::I2C1::steal();
            let scl = peripherals::PIN_7::steal();
            let sda = peripherals::PIN_6::steal();

            let mut cfg = i2c::Config::default();
            cfg.frequency = self.config.frequency_hz;
            cfg.sda_pullup = true;
            cfg.scl_pullup = true;

            RpI2c::new_async(peri, scl, sda, irqs, cfg)
        }
    }
}
