use crate::error::BusError;

/// I2C device I/O contract.
///
/// All methods take an explicit 7-bit `address` parameter.
/// Implementors are expected to add timeout, retry, and statistics tracking.
#[allow(async_fn_in_trait)]
pub trait DeviceIo {
    /// Write a single register value to `reg`.
    async fn write_reg(&mut self, address: u8, reg: u8, value: u8) -> Result<(), BusError>;

    /// Read a single register value from `reg`.
    async fn read_reg(&mut self, address: u8, reg: u8) -> Result<u8, BusError>;

    /// Read a sequence of registers starting at `start_reg` into `buf`.
    async fn read_regs(
        &mut self,
        address: u8,
        start_reg: u8,
        buf: &mut [u8],
    ) -> Result<(), BusError>;

    /// Write-then-read with explicit buffers.
    ///
    /// This is the most general primitive. For simple register access,
    /// prefer [`write_reg`](DeviceIo::write_reg) /
    /// [`read_reg`](DeviceIo::read_reg) /
    /// [`read_regs`](DeviceIo::read_regs).
    async fn write_read(
        &mut self,
        address: u8,
        write_buf: &[u8],
        read_buf: &mut [u8],
    ) -> Result<(), BusError>;
}
