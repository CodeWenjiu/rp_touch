use embedded_hal_async::i2c::I2c as _;

use crate::types::Error;

use super::Ft3168;

impl<'d> Ft3168<'d> {
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
}
