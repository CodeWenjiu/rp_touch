use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{
    Peri,
    gpio::Input,
    i2c::{Async, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};

use crate::{
    regs::{
        CTRL1_ADDR_AI, CTRL1_BASE, CTRL1_FIFO_INT_SEL, CTRL1_INT1_EN,
        CTRL2_DEFAULT_ACCEL_8G_1000HZ, CTRL3_DEFAULT_GYRO_512DPS_1000HZ, CTRL7_ACCEL_ENABLE,
        CTRL7_GYRO_ENABLE, CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT, CTRL9_CMD_ACK, CTRL9_CMD_RST_FIFO,
        QMI8658_CHIP_ID, QMI8658_REG_CTRL1, QMI8658_REG_CTRL2, QMI8658_REG_CTRL3,
        QMI8658_REG_CTRL7, QMI8658_REG_CTRL8, QMI8658_REG_CTRL9, QMI8658_REG_FIFO_CTRL,
        QMI8658_REG_FIFO_WTM_TH, QMI8658_REG_TEMP_L, QMI8658_REG_RESET, QMI8658_REG_STATUSINT, QMI8658_REG_WHO_AM_I,
        RESET_SOFT_CMD,
    },
    types::{Error, FifoConfig, ImuReport, Qmi8658Config},
};

mod io;
mod stream;

pub struct Qmi8658<'d> {
    i2c: SharedI2cDevice<'d>,
    int1: Input<'d>,
    address: u8,
    fifo_ctrl_cfg: u8,
}

type SharedBusInner<'d> = I2c<'d, peripherals::I2C1, Async>;
pub type SharedI2cBus<'d> = Mutex<CriticalSectionRawMutex, SharedBusInner<'d>>;
type SharedI2cDevice<'d> = I2cDevice<'d, CriticalSectionRawMutex, SharedBusInner<'d>>;

impl<'d> Qmi8658<'d> {
    const INIT_BOOT_WAIT_MS: u64 = 15;
    const SOFT_RESET_WAIT_MS: u64 = 30;
    const RESET_SETTLE_WAIT_MS: u64 = 2;
    const WHO_AM_I_RETRIES: usize = 30;
    const WHO_AM_I_RETRY_WAIT_MS: u64 = 2;
    const INIT_ATTEMPTS: usize = 4;
    const INIT_RETRY_WAIT_MS: u64 = 10;

    pub fn new_shared(
        i2c_bus: &'d SharedI2cBus<'d>,
        int1: Peri<'d, peripherals::PIN_8>,
        config: Qmi8658Config,
    ) -> Result<Self, Error> {
        if config.address >= 0x80 {
            return Err(Error::InvalidAddress(config.address));
        }

        let int1 = Input::new(int1, config.int1_pull);

        Ok(Self {
            i2c: I2cDevice::new(i2c_bus),
            int1,
            address: config.address,
            fifo_ctrl_cfg: 0,
        })
    }

    pub async fn init(&mut self) -> Result<u8, Error> {
        // Give the IMU enough boot time before the first I2C access.
        Timer::after(Duration::from_millis(Self::INIT_BOOT_WAIT_MS)).await;

        let mut last_error = None;
        for _ in 0..Self::INIT_ATTEMPTS {
            self.pre_reset_cleanup().await;

            match self.init_once().await {
                Ok(chip_id) => return Ok(chip_id),
                Err(err) => {
                    last_error = Some(err);
                    Timer::after(Duration::from_millis(Self::INIT_RETRY_WAIT_MS)).await;
                }
            }
        }

        Err(last_error.unwrap_or(Error::InvalidChipId(0)))
    }

    pub async fn device_id(&mut self) -> Result<u8, Error> {
        self.read_reg(QMI8658_REG_WHO_AM_I).await
    }

    pub async fn enable_accel_gyro(&mut self) -> Result<(), Error> {
        self.write_reg_checked(QMI8658_REG_CTRL2, CTRL2_DEFAULT_ACCEL_8G_1000HZ, 0xFF)
            .await?;
        self.write_reg_checked(QMI8658_REG_CTRL3, CTRL3_DEFAULT_GYRO_512DPS_1000HZ, 0xFF)
            .await?;

        self.write_reg_checked(
            QMI8658_REG_CTRL7,
            CTRL7_ACCEL_ENABLE | CTRL7_GYRO_ENABLE,
            CTRL7_ACCEL_ENABLE | CTRL7_GYRO_ENABLE,
        )
        .await
    }

    pub async fn enable_fifo_wtm_int1(&mut self, config: FifoConfig) -> Result<(), Error> {
        self.write_reg_checked(
            QMI8658_REG_CTRL1,
            CTRL1_BASE | CTRL1_ADDR_AI | CTRL1_INT1_EN | CTRL1_FIFO_INT_SEL,
            CTRL1_ADDR_AI | CTRL1_INT1_EN | CTRL1_FIFO_INT_SEL,
        )
        .await?;
        self.write_reg_checked(
            QMI8658_REG_CTRL8,
            CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT,
            CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT,
        )
        .await?;
        self.write_reg_checked(QMI8658_REG_FIFO_WTM_TH, config.watermark_odr_samples, 0xFF)
            .await?;

        self.fifo_ctrl_cfg = ((config.size.bits() << crate::regs::FIFO_CTRL_SIZE_SHIFT) & 0b1100)
            | (config.mode.bits() & crate::regs::FIFO_CTRL_MODE_MASK);
        self.write_reg_checked(QMI8658_REG_FIFO_CTRL, self.fifo_ctrl_cfg, 0x0F)
            .await?;

        self.ctrl9_command(CTRL9_CMD_RST_FIFO).await?;
        self.enable_accel_gyro().await
    }

    pub async fn setup_int1_fifo_stream(&mut self, config: FifoConfig) -> Result<(), ImuReport> {
        self.init().await.map_err(|e| match e {
            Error::InvalidChipId(chip_id) => ImuReport::InvalidChipId(chip_id),
            _ => ImuReport::InitError,
        })?;
        self.enable_fifo_wtm_int1(config)
            .await
            .map_err(|_| ImuReport::FifoConfigError)
    }

    pub async fn read_temperature(&mut self) -> Result<i32, Error> {
        let mut buf = [0u8; 2];
        self.read_regs(QMI8658_REG_TEMP_L, &mut buf).await?;
        let raw = i16::from_le_bytes([buf[0], buf[1]]);
        Ok(raw as i32 * 10 / 256)
    }

    pub async fn soft_reset(&mut self) -> Result<(), Error> {
        self.write_reg(QMI8658_REG_RESET, RESET_SOFT_CMD).await?;
        Timer::after(Duration::from_millis(Self::SOFT_RESET_WAIT_MS)).await;

        // Best-effort cleanup for command/status latches after reset.
        let _ = self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK).await;
        let _ = self.read_reg(QMI8658_REG_STATUSINT).await;
        Timer::after(Duration::from_millis(Self::RESET_SETTLE_WAIT_MS)).await;

        Ok(())
    }

    pub async fn wait_int1_rising_edge(&mut self) {
        self.int1.wait_for_rising_edge().await;
    }

    pub async fn wait_int1_any_edge(&mut self) {
        self.int1.wait_for_any_edge().await;
    }

    pub fn int1_is_high(&self) -> bool {
        self.int1.is_high()
    }

    async fn init_once(&mut self) -> Result<u8, Error> {
        self.soft_reset().await?;
        self.wait_for_who_am_i().await
    }

    async fn wait_for_who_am_i(&mut self) -> Result<u8, Error> {
        let mut last_error = Error::InvalidChipId(0);
        for _ in 0..Self::WHO_AM_I_RETRIES {
            match self.device_id().await {
                Ok(who_am_i) if who_am_i == QMI8658_CHIP_ID => return Ok(who_am_i),
                Ok(who_am_i) => {
                    last_error = Error::InvalidChipId(who_am_i);
                }
                Err(err) => {
                    last_error = err;
                }
            }
            Timer::after(Duration::from_millis(Self::WHO_AM_I_RETRY_WAIT_MS)).await;
        }
        Err(last_error)
    }

    async fn pre_reset_cleanup(&mut self) {
        let _ = self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK).await;
        let _ = self.write_reg(QMI8658_REG_CTRL7, 0).await;
        let _ = self.write_reg(QMI8658_REG_FIFO_CTRL, 0).await;
        let _ = self.read_reg(QMI8658_REG_STATUSINT).await;
    }

    async fn write_reg_checked(&mut self, reg: u8, value: u8, mask: u8) -> Result<(), Error> {
        self.write_reg(reg, value).await?;
        let actual = self.read_reg(reg).await?;
        if (actual & mask) != (value & mask) {
            return Err(Error::RegisterVerify {
                reg,
                expected: value,
                actual,
            });
        }
        Ok(())
    }
}
