use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

/// Per-operation statistics tracked lock-free for telemetry.
pub struct BusStats {
    /// Total I2C operations attempted (writes + reads).
    pub total_ops: AtomicU32,
    /// Operations that succeeded on the first try.
    pub successful_ops: AtomicU32,
    /// Operations that succeeded after at least one retry.
    pub retried_ops: AtomicU32,
    /// Operations that timed out (did not complete within the deadline).
    pub timed_out_ops: AtomicU32,
    /// Operations that failed (after exhausting retries, or non-retryable errors).
    pub error_ops: AtomicU32,
    /// Number of bus recovery events triggered.
    pub recovery_count: AtomicU32,
    /// Consecutive error count — resets to 0 on success.
    pub consecutive_errors: AtomicU8,
}

impl Default for BusStats {
    fn default() -> Self {
        Self::new()
    }
}

impl BusStats {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            total_ops: AtomicU32::new(0),
            successful_ops: AtomicU32::new(0),
            retried_ops: AtomicU32::new(0),
            timed_out_ops: AtomicU32::new(0),
            error_ops: AtomicU32::new(0),
            recovery_count: AtomicU32::new(0),
            consecutive_errors: AtomicU8::new(0),
        }
    }

    /// Atomically snapshot all counters for telemetry.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        let total = self.total_ops.load(Ordering::Relaxed);
        let successful = self.successful_ops.load(Ordering::Relaxed);
        let health_pct = if total > 0 {
            ((successful as u64 * 100) / total as u64) as u8
        } else {
            100
        };
        StatsSnapshot {
            total_ops: total,
            successful_ops: successful,
            retried_ops: self.retried_ops.load(Ordering::Relaxed),
            timed_out_ops: self.timed_out_ops.load(Ordering::Relaxed),
            error_ops: self.error_ops.load(Ordering::Relaxed),
            recovery_count: self.recovery_count.load(Ordering::Relaxed),
            consecutive_errors: self.consecutive_errors.load(Ordering::Relaxed),
            health_pct,
        }
    }
}

/// Point-in-time snapshot of bus statistics (owned, Copy).
#[derive(Clone, Copy, Debug, Default)]
pub struct StatsSnapshot {
    pub total_ops: u32,
    pub successful_ops: u32,
    pub retried_ops: u32,
    pub timed_out_ops: u32,
    pub error_ops: u32,
    pub recovery_count: u32,
    pub consecutive_errors: u8,
    /// Health percentage: `successful_ops * 100 / total_ops` (100 if no ops yet).
    pub health_pct: u8,
}

/// Configuration for an I2C bus instance.
#[derive(Clone, Copy, Debug)]
pub struct BusConfig {
    /// I2C bus clock frequency in Hz (typically 100_000 or 400_000).
    pub frequency_hz: u32,
    /// SCL pin GPIO number.
    pub scl_pin: u8,
    /// SDA pin GPIO number.
    pub sda_pin: u8,
    /// Default per-operation timeout in milliseconds.
    pub default_timeout_ms: u64,
    /// Maximum retry count for transient errors (NoAcknowledge, ArbitrationLoss).
    pub max_retries: u8,
    /// Number of SCL pulses to generate during bus recovery.
    pub pulse_count: usize,
    /// Half-cycle delay in microseconds during recovery bit-banging.
    pub bus_delay_us: u32,
    /// Reset settle delay in microseconds.
    pub reset_delay_us: u32,
    /// Post-recovery cooldown in milliseconds before normal operation resumes.
    pub recovery_cooldown_ms: u64,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 400_000,
            scl_pin: 7,
            sda_pin: 6,
            default_timeout_ms: 10,
            max_retries: 3,
            pulse_count: 9,
            bus_delay_us: 3,
            reset_delay_us: 10,
            recovery_cooldown_ms: 300,
        }
    }
}
