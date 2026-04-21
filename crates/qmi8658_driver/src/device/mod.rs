use embassy_rp::{
    Peri, bind_interrupts,
    gpio::Input,
    i2c::{self, Async, Config as I2cConfig, I2c},
    peripherals,
};
use embassy_time::{Duration, Timer};

use crate::{
    regs::{
        CTRL1_ADDR_AI, CTRL1_BASE, CTRL1_FIFO_INT_SEL, CTRL1_INT1_EN, CTRL2_DEFAULT_ACCEL_8G_1000HZ,
        CTRL3_DEFAULT_GYRO_512DPS_1000HZ, CTRL7_ACCEL_ENABLE, CTRL7_GYRO_ENABLE,
        CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT, CTRL9_CMD_RST_FIFO, QMI8658_CHIP_ID, QMI8658_REG_CTRL1,
        QMI8658_REG_CTRL2, QMI8658_REG_CTRL3, QMI8658_REG_CTRL7, QMI8658_REG_CTRL8, QMI8658_REG_FIFO_CTRL,
        QMI8658_REG_FIFO_WTM_TH, QMI8658_REG_WHO_AM_I,
    },
    types::{Error, FifoConfig, ImuReport, Qmi8658Config},
};

mod io;
mod stream;

bind_interrupts!(struct Irqs {
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
});

pub struct Qmi8658<'d> {
    i2c: I2c<'d, peripherals::I2C1, Async>,
    int1: Input<'d>,
    address: u8,
    fifo_ctrl_cfg: u8,
}

impl<'d> Qmi8658<'d> {
    pub fn new(
        i2c: Peri<'d, peripherals::I2C1>,
        sda: Peri<'d, peripherals::PIN_6>,
        scl: Peri<'d, peripherals::PIN_7>,
        int1: Peri<'d, peripherals::PIN_8>,
        config: Qmi8658Config,
    ) -> Result<Self, Error> {
        if config.address >= 0x80 {
            return Err(Error::InvalidAddress(config.address));
        }

        let mut i2c_config = I2cConfig::default();
        i2c_config.frequency = config.i2c_frequency_hz;
        i2c_config.sda_pullup = true;
        i2c_config.scl_pullup = true;

        let i2c = I2c::new_async(i2c, scl, sda, Irqs, i2c_config);
        let int1 = Input::new(int1, config.int1_pull);

        Ok(Self {
            i2c,
            int1,
            address: config.address,
            fifo_ctrl_cfg: 0,
        })
    }

    pub fn new_default(
        i2c: Peri<'d, peripherals::I2C1>,
        sda: Peri<'d, peripherals::PIN_6>,
        scl: Peri<'d, peripherals::PIN_7>,
        int1: Peri<'d, peripherals::PIN_8>,
    ) -> Result<Self, Error> {
        Self::new(i2c, sda, scl, int1, Qmi8658Config::default())
    }

    pub async fn init(&mut self) -> Result<u8, Error> {
        // Give the IMU enough boot time before the first I2C access.
        Timer::after(Duration::from_millis(15)).await;

        let who_am_i = self.device_id().await?;
        if who_am_i != QMI8658_CHIP_ID {
            return Err(Error::InvalidChipId(who_am_i));
        }

        Ok(who_am_i)
    }

    pub async fn device_id(&mut self) -> Result<u8, Error> {
        self.read_reg(QMI8658_REG_WHO_AM_I).await
    }

    pub async fn enable_accel_gyro(&mut self) -> Result<(), Error> {
        self.write_reg(QMI8658_REG_CTRL2, CTRL2_DEFAULT_ACCEL_8G_1000HZ)
            .await?;
        self.write_reg(QMI8658_REG_CTRL3, CTRL3_DEFAULT_GYRO_512DPS_1000HZ)
            .await?;

        self.write_reg(QMI8658_REG_CTRL7, CTRL7_ACCEL_ENABLE | CTRL7_GYRO_ENABLE)
            .await
    }

    pub async fn enable_fifo_wtm_int1(&mut self, config: FifoConfig) -> Result<(), Error> {
        self.write_reg(
            QMI8658_REG_CTRL1,
            CTRL1_BASE | CTRL1_ADDR_AI | CTRL1_INT1_EN | CTRL1_FIFO_INT_SEL,
        )
        .await?;
        self.write_reg(QMI8658_REG_CTRL8, CTRL8_CTRL9_HANDSHAKE_USE_STATUSINT)
            .await?;
        self.write_reg(QMI8658_REG_FIFO_WTM_TH, config.watermark_odr_samples)
            .await?;

        self.fifo_ctrl_cfg = ((config.size.bits() << crate::regs::FIFO_CTRL_SIZE_SHIFT) & 0b1100)
            | (config.mode.bits() & crate::regs::FIFO_CTRL_MODE_MASK);
        self.write_reg(QMI8658_REG_FIFO_CTRL, self.fifo_ctrl_cfg).await?;

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

    pub async fn wait_int1_rising_edge(&mut self) {
        self.int1.wait_for_rising_edge().await;
    }

    pub fn int1_is_high(&self) -> bool {
        self.int1.is_high()
    }
}
