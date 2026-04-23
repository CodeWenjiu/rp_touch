use core::fmt;

use micromath::F32Ext;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ImuRawSample {
    pub accel: [i16; 3],
    pub gyro: [i16; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ImuTiltAngles {
    pub pitch_deg: f32,
    pub roll_deg: f32,
}

impl fmt::Display for ImuTiltAngles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pitch: {:.2},roll: {:.2}", self.pitch_deg, self.roll_deg)
    }
}

const RAD_TO_DEG: f32 = 57.295_78_f32;
const ACCEL_LSB_PER_G_8G: f32 = 4096.0_f32;
const GYRO_LSB_PER_DPS_512: f32 = 64.0_f32;

impl ImuRawSample {
    #[inline]
    pub fn accel_g_8g(&self) -> [f32; 3] {
        [
            self.accel[0] as f32 / ACCEL_LSB_PER_G_8G,
            self.accel[1] as f32 / ACCEL_LSB_PER_G_8G,
            self.accel[2] as f32 / ACCEL_LSB_PER_G_8G,
        ]
    }

    #[inline]
    pub fn gyro_dps_512(&self) -> [f32; 3] {
        [
            self.gyro[0] as f32 / GYRO_LSB_PER_DPS_512,
            self.gyro[1] as f32 / GYRO_LSB_PER_DPS_512,
            self.gyro[2] as f32 / GYRO_LSB_PER_DPS_512,
        ]
    }

    #[inline]
    pub fn tilt_deg_from_accel_8g(&self) -> ImuTiltAngles {
        // Static tilt estimation from gravity vector (accelerometer only).
        // Use f32 end-to-end to stay on the hard-float path for arithmetic.
        let [ax, ay, az] = self.accel_g_8g();

        let pitch_deg = ax.atan2((ay * ay + az * az).sqrt()) * RAD_TO_DEG;
        let roll_deg = ay.atan2(az) * RAD_TO_DEG;

        ImuTiltAngles {
            pitch_deg,
            roll_deg,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ImuFrame {
    pub sample: ImuRawSample,
}

impl fmt::Display for ImuFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IMU,{},{},{},{},{},{}",
            self.sample.accel[0],
            self.sample.accel[1],
            self.sample.accel[2],
            self.sample.gyro[0],
            self.sample.gyro[1],
            self.sample.gyro[2]
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuReport {
    Sample(ImuRawSample),
    ReadError,
    InitError,
    InvalidChipId(u8),
    FifoConfigError,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Int1FifoStreamState;
