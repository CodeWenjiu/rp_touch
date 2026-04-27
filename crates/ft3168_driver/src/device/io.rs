use embassy_time::{Duration, with_timeout};
use embedded_hal_async::i2c::I2c as _;

use crate::types::Error;

use super::Ft3168;

impl<'d> Ft3168<'d> {
    // Keep touch transactions short so they cannot block IMU polling
    // for tens of milliseconds in the shared sensor task.
    const I2C_OP_TIMEOUT_MS: u64 = 3;

    pub async fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), Error> {
        let bytes = [reg, value];
        with_timeout(
            Duration::from_millis(Self::I2C_OP_TIMEOUT_MS),
            self.i2c.write(self.address, &bytes),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        Ok(())
    }

    pub async fn read_reg(&mut self, reg: u8) -> Result<u8, Error> {
        let mut out = [0u8; 1];
        self.read_regs(reg, &mut out).await?;
        Ok(out[0])
    }

    pub async fn read_regs(&mut self, start_reg: u8, out: &mut [u8]) -> Result<(), Error> {
        with_timeout(
            Duration::from_millis(Self::I2C_OP_TIMEOUT_MS),
            self.i2c.write_read(self.address, &[start_reg], out),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        Ok(())
    }
}
