use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_rp::i2c::{self, AbortReason};

/// I2C bus-level error produced by [`RetryingDevice`](crate::RetryingDevice).
///
/// Drivers should convert this into their own error type via
/// `From<BusError>` or a dedicated variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusError {
    /// Transient I2C bus error from the peripheral.
    /// May be retryable (see [`BusError::is_retryable`]).
    I2c(i2c::Error),
    /// The shared-bus lock could not be acquired or held.
    BusLock,
    /// The overall operation timed out (deadline expired).
    Timeout,
    /// A retryable error exhausted retries and is now treated as fatal.
    Fatal(i2c::Error),
}

impl From<i2c::Error> for BusError {
    fn from(e: i2c::Error) -> Self {
        Self::I2c(e)
    }
}

impl From<I2cDeviceError<i2c::Error>> for BusError {
    fn from(_: I2cDeviceError<i2c::Error>) -> Self {
        Self::BusLock
    }
}

impl BusError {
    /// Returns `true` if the error is likely transient and retrying may succeed.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::I2c(i2c::Error::Abort(AbortReason::NoAcknowledge))
                | Self::I2c(i2c::Error::Abort(AbortReason::ArbitrationLoss))
        )
    }

    /// Extract the inner `i2c::Error` if present.
    #[must_use]
    pub fn into_i2c_error(self) -> i2c::Error {
        match self {
            Self::I2c(e) | Self::Fatal(e) => e,
            Self::BusLock | Self::Timeout => i2c::Error::Abort(AbortReason::TxNotEmpty(0)),
        }
    }
}
