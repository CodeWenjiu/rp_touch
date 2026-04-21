use core::fmt::Write;

use crate::types::{ImuRawSample, ImuReport};

pub fn format_report_line(report: ImuReport) -> heapless::String<64> {
    let mut line = heapless::String::<64>::new();
    match report {
        ImuReport::Sample(sample) => {
            let _ = write!(
                line,
                "IMU,{},{},{},{},{},{}\r\n",
                sample.accel[0],
                sample.accel[1],
                sample.accel[2],
                sample.gyro[0],
                sample.gyro[1],
                sample.gyro[2]
            );
        }
        ImuReport::ReadError(count) => {
            let _ = write!(line, "IMU_ERR,read_fail_count={}\r\n", count);
        }
        ImuReport::InitError => {
            let _ = line.push_str("IMU_ERR,init_failed\r\n");
        }
        ImuReport::FifoConfigError => {
            let _ = line.push_str("IMU_ERR,fifo_config_failed\r\n");
        }
        ImuReport::InvalidChipId(chip_id) => {
            let _ = write!(line, "IMU_ERR,chip_id=0x{:02X}\r\n", chip_id);
        }
    }
    line
}

pub fn format_sample_line(sample: ImuRawSample) -> heapless::String<64> {
    format_report_line(ImuReport::Sample(sample))
}
