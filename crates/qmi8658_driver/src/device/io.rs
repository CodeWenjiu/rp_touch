use embedded_hal_async::i2c::I2c as _;

use crate::{
    regs::{
        CTRL9_CMD_ACK, QMI8658_REG_CTRL9, QMI8658_REG_FIFO_DATA, QMI8658_REG_STATUSINT,
        STATUSINT_CMD_DONE,
    },
    types::Error,
};

use super::Qmi8658;

impl<'d> Qmi8658<'d> {
    pub async fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), Error> {
        let bytes = [reg, value];
        self.i2c.write(self.address, &bytes).await?;
        Ok(())
    }

    pub async fn read_reg(&mut self, reg: u8) -> Result<u8, Error> {
        let mut out = [0u8; 1];
        self.read_regs(reg, &mut out).await?;
        Ok(out[0])
    }

    pub async fn read_regs(&mut self, start_reg: u8, out: &mut [u8]) -> Result<(), Error> {
        self.i2c.write_read(self.address, &[start_reg], out).await?;
        Ok(())
    }

    pub(super) async fn read_fifo_bytes(&mut self, out: &mut [u8]) -> Result<(), Error> {
        self.i2c
            .write_read(self.address, &[QMI8658_REG_FIFO_DATA], out)
            .await?;
        Ok(())
    }

    pub(super) async fn ctrl9_command(&mut self, cmd: u8) -> Result<(), Error> {
        self.write_reg(QMI8658_REG_CTRL9, cmd).await?;

        let mut tries = 0usize;
        while tries < 200 {
            let status = self.read_reg(QMI8658_REG_STATUSINT).await?;
            if (status & STATUSINT_CMD_DONE) != 0 {
                self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK).await?;
                return Ok(());
            }
            tries += 1;
        }

        let _ = self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK).await;
        Err(Error::Ctrl9Timeout)
    }
}
