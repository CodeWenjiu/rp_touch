#![no_std]

use core::fmt::{self, Write};

pub const FRAME_PREFIX: &str = "RP_IMU";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TelemetryFrame {
    pub accel: [i16; 3],
    pub gyro: [i16; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeformatError {
    InvalidPrefix,
    InvalidFieldCount,
    InvalidNumber,
}

impl TelemetryFrame {
    pub const fn new(accel: [i16; 3], gyro: [i16; 3]) -> Self {
        Self { accel, gyro }
    }

    pub fn format<const N: usize>(&self) -> Result<heapless::String<N>, FormatError> {
        let mut out = heapless::String::new();
        self.format_into(&mut out)?;
        Ok(out)
    }

    pub fn format_into<const N: usize>(
        &self,
        out: &mut heapless::String<N>,
    ) -> Result<(), FormatError> {
        out.clear();
        write!(
            out,
            "{FRAME_PREFIX},{},{},{},{},{},{}",
            self.accel[0], self.accel[1], self.accel[2], self.gyro[0], self.gyro[1], self.gyro[2]
        )
        .map_err(|_| FormatError::BufferTooSmall)
    }

    pub fn deformat(input: &str) -> Result<Self, DeformatError> {
        let line = input.trim();
        let tail = line
            .strip_prefix(FRAME_PREFIX)
            .and_then(|v| v.strip_prefix(','))
            .ok_or(DeformatError::InvalidPrefix)?;

        let mut fields = tail.split(',');

        let mut parse_i16 = || -> Result<i16, DeformatError> {
            let raw = fields.next().ok_or(DeformatError::InvalidFieldCount)?;
            raw.parse::<i16>().map_err(|_| DeformatError::InvalidNumber)
        };

        let accel = [parse_i16()?, parse_i16()?, parse_i16()?];
        let gyro = [parse_i16()?, parse_i16()?, parse_i16()?];

        if fields.next().is_some() {
            return Err(DeformatError::InvalidFieldCount);
        }

        Ok(Self { accel, gyro })
    }
}

impl fmt::Display for TelemetryFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{FRAME_PREFIX},{},{},{},{},{},{}",
            self.accel[0], self.accel[1], self.accel[2], self.gyro[0], self.gyro[1], self.gyro[2]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::TelemetryFrame;

    #[test]
    fn format_and_deformat_roundtrip() {
        let frame = TelemetryFrame::new([123, -456, 789], [-1000, 2000, -3000]);
        let text = frame.format::<64>().expect("format");
        let parsed = TelemetryFrame::deformat(text.as_str()).expect("deformat");
        assert_eq!(parsed.accel, [123, -456, 789]);
        assert_eq!(parsed.gyro, [-1000, 2000, -3000]);
    }
}
