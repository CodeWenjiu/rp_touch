use embassy_time::{Duration, Timer};
use i2c_bus::DeviceIo;

use crate::{
    regs::{
        FT3168_BOOT_WAIT_MS, FT3168_REG_CHIP_ID, FT3168_REG_FIRMWARE_ID,
        FT3168_REG_TOUCH_STATUS, FT3168_RETRY_COUNT, FT3168_TOUCH_POINT_BYTES,
    },
    types::{Error, Ft3168Config, TouchPoint, TouchSample},
};

/// FT3168 capacitive touch controller driver.
pub struct Ft3168<'d, IO: DeviceIo> {
    i2c: IO,
    address: u8,
    _phantom: core::marker::PhantomData<&'d ()>,
}

impl<'d, IO: DeviceIo> Ft3168<'d, IO> {
    pub fn new(i2c: IO, config: Ft3168Config) -> Result<Self, Error> {
        if config.address >= 0x80 {
            return Err(Error::InvalidAddress(config.address));
        }

        Ok(Self {
            i2c,
            address: config.address,
            _phantom: core::marker::PhantomData,
        })
    }

    pub async fn init(&mut self) -> Result<u8, Error> {
        Timer::after(Duration::from_millis(FT3168_BOOT_WAIT_MS)).await;

        let mut chip_id = 0u8;
        for _ in 0..FT3168_RETRY_COUNT {
            chip_id = self.read_reg(FT3168_REG_CHIP_ID).await?;
            if chip_id != 0 && chip_id != 0xFF {
                return Ok(chip_id);
            }
            Timer::after(Duration::from_millis(2)).await;
        }
        Err(Error::InvalidChipId(chip_id))
    }

    pub async fn firmware_id(&mut self) -> Result<u8, Error> {
        self.read_reg(FT3168_REG_FIRMWARE_ID).await
    }

    pub async fn read_touch_sample(&mut self) -> Result<TouchSample, Error> {
        let mut raw = [0u8; 1 + FT3168_TOUCH_POINT_BYTES];
        self.read_regs(FT3168_REG_TOUCH_STATUS, &mut raw)
            .await?;

        let touched = (raw[0] & 0x0F) > 0;
        if !touched {
            return Ok(None);
        }

        let base = 1;
        let xh = raw[base];
        let xl = raw[base + 1];
        let yh = raw[base + 2];
        let yl = raw[base + 3];

        let x = (((xh & 0x0F) as u16) << 8) | xl as u16;
        let y = (((yh & 0x0F) as u16) << 8) | yl as u16;

        Ok(Some(TouchPoint { x, y }))
    }

    // ── I2C helpers (thin wrappers around DeviceIo) ──────────────────────

    async fn read_reg(&mut self, reg: u8) -> Result<u8, Error> {
        self.i2c
            .read_reg(self.address, reg)
            .await
            .map_err(Error::Bus)
    }

    async fn read_regs(&mut self, start_reg: u8, out: &mut [u8]) -> Result<(), Error> {
        self.i2c
            .read_regs(self.address, start_reg, out)
            .await
            .map_err(Error::Bus)
    }
}
