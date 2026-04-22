use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{
    i2c::{Async, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};

use crate::{
    regs::{
        FT3168_BOOT_WAIT_MS, FT3168_MAX_TOUCH_POINTS, FT3168_REG_CHIP_ID, FT3168_REG_TOUCH_STATUS,
        FT3168_RETRY_COUNT, FT3168_TOUCH_POINT_BYTES,
    },
    types::{Error, Ft3168Config, TouchEvent, TouchPoint, TouchSample},
};

mod io;

pub struct Ft3168<'d> {
    i2c: SharedI2cDevice<'d>,
    address: u8,
}

type SharedBusInner<'d> = I2c<'d, peripherals::I2C1, Async>;
pub type SharedI2cBus<'d> = Mutex<NoopRawMutex, SharedBusInner<'d>>;
type SharedI2cDevice<'d> = I2cDevice<'d, NoopRawMutex, SharedBusInner<'d>>;

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
        let mut raw = [0u8; 1 + FT3168_MAX_TOUCH_POINTS * FT3168_TOUCH_POINT_BYTES];
        self.read_regs(FT3168_REG_TOUCH_STATUS, &mut raw).await?;

        let count = (raw[0] & 0x0F).min(FT3168_MAX_TOUCH_POINTS as u8);
        let mut sample = TouchSample {
            touch_count: count,
            points: [TouchPoint::default(); FT3168_MAX_TOUCH_POINTS],
        };

        for i in 0..count as usize {
            let base = 1 + i * FT3168_TOUCH_POINT_BYTES;
            let xh = raw[base];
            let xl = raw[base + 1];
            let yh = raw[base + 2];
            let yl = raw[base + 3];
            let weight = raw[base + 4];
            let misc = raw[base + 5];

            let x = (((xh & 0x0F) as u16) << 8) | xl as u16;
            let y = (((yh & 0x0F) as u16) << 8) | yl as u16;
            let event = TouchEvent::from_bits((xh >> 6) & 0x03);
            let id = (yh >> 4) & 0x0F;
            let area = misc >> 4;

            sample.points[i] = TouchPoint {
                id,
                event: Some(event),
                x,
                y,
                weight,
                area,
            };
        }

        Ok(sample)
    }
}
