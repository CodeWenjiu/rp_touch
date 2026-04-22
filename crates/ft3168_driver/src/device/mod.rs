use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{
    i2c::{Async, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};

use crate::{
    regs::{
        FT3168_BOOT_WAIT_MS, FT3168_REG_CHIP_ID, FT3168_REG_TOUCH_STATUS, FT3168_RETRY_COUNT,
        FT3168_TOUCH_POINT_BYTES,
    },
    types::{Error, Ft3168Config, TouchPoint, TouchSample},
};

mod io;

pub struct Ft3168<'d> {
    i2c: SharedI2cDevice<'d>,
    address: u8,
}

type SharedBusInner<'d> = I2c<'d, peripherals::I2C1, Async>;
pub type SharedI2cBus<'d> = Mutex<CriticalSectionRawMutex, SharedBusInner<'d>>;
type SharedI2cDevice<'d> = I2cDevice<'d, CriticalSectionRawMutex, SharedBusInner<'d>>;

impl<'d> Ft3168<'d> {
    pub fn new_shared(i2c_bus: &'d SharedI2cBus<'d>, config: Ft3168Config) -> Result<Self, Error> {
        if config.address >= 0x80 {
            return Err(Error::InvalidAddress(config.address));
        }

        Ok(Self {
            i2c: I2cDevice::new(i2c_bus),
            address: config.address,
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
        Ok(chip_id)
    }

    pub async fn firmware_id(&mut self) -> Result<u8, Error> {
        self.read_reg(crate::regs::FT3168_REG_FIRMWARE_ID).await
    }

    pub async fn read_touch_sample(&mut self) -> Result<TouchSample, Error> {
        let mut raw = [0u8; 1 + FT3168_TOUCH_POINT_BYTES];
        self.read_regs(FT3168_REG_TOUCH_STATUS, &mut raw).await?;

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
}
