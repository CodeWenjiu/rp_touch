use i2c_bus::DeviceIo;

use crate::{
    regs::{
        CTRL9_CMD_ACK, QMI8658_REG_CTRL9, QMI8658_REG_FIFO_DATA,
        QMI8658_REG_STATUSINT, STATUSINT_CMD_DONE,
    },
    types::Error,
};

use super::Qmi8658;

impl<'d, IO: DeviceIo> Qmi8658<'d, IO> {
    /// Read raw bytes from the FIFO data register.
    pub(super) async fn read_fifo_bytes(&mut self, out: &mut [u8]) -> Result<(), Error> {
        self.i2c
            .write_read(self.address, &[QMI8658_REG_FIFO_DATA], out)
            .await
            .map_err(Error::Bus)
    }

    /// Send a CTRL9 command with handshake polling.
    ///
    /// Writes `cmd` to CTRL9, then polls STATUSINT until CMD_DONE is set
    /// (up to 200 iterations), and sends the ACK command.
    pub(super) async fn ctrl9_command(&mut self, cmd: u8) -> Result<(), Error> {
        self.write_reg(QMI8658_REG_CTRL9, cmd).await?;

        let mut tries = 0usize;
        while tries < 200 {
            let status = self.read_reg(QMI8658_REG_STATUSINT).await?;
            if (status & STATUSINT_CMD_DONE) != 0 {
                self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK)
                    .await?;
                return Ok(());
            }
            tries += 1;
        }

        let _ = self.write_reg(QMI8658_REG_CTRL9, CTRL9_CMD_ACK).await;
        Err(Error::Ctrl9Timeout)
    }
}
