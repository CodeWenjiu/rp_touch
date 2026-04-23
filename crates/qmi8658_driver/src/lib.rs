#![no_std]

mod device;
mod format;
mod regs;
mod storage;
mod task;
mod types;

pub use device::{Qmi8658, SharedI2cBus};
pub use format::{format_report_line, format_sample_line};
pub use regs::{
    QMI8658_CHIP_ID, QMI8658_I2C_ADDR, QMI8658_REG_AX_L, QMI8658_REG_CTRL1, QMI8658_REG_CTRL2,
    QMI8658_REG_CTRL3, QMI8658_REG_CTRL7, QMI8658_REG_CTRL8, QMI8658_REG_CTRL9,
    QMI8658_REG_FIFO_CTRL, QMI8658_REG_FIFO_DATA, QMI8658_REG_FIFO_SMPL_CNT,
    QMI8658_REG_FIFO_STATUS, QMI8658_REG_FIFO_WTM_TH, QMI8658_REG_STATUSINT, QMI8658_REG_WHO_AM_I,
};
pub use storage::{ImuPipeline, ImuReader};
pub use task::imu_capture_task;
pub use types::{
    CaptureState, CaptureStats, Error, FifoConfig, FifoMode, FifoSize, ImuFrame, ImuRawSample,
    ImuReport, ImuTiltAngles, Int1FifoStreamState, Qmi8658Config,
};
