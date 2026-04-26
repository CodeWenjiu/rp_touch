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
    pub yaw_deg: f32,
}

impl fmt::Display for ImuTiltAngles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pitch: {:.2},roll: {:.2},yaw: {:.2}",
            self.pitch_deg, self.roll_deg, self.yaw_deg
        )
    }
}

const RAD_TO_DEG: f32 = 57.295_78_f32;
const DEG_TO_RAD: f32 = 0.017_453_292_f32;
const ACCEL_LSB_PER_G_8G: f32 = 4096.0_f32;
const GYRO_LSB_PER_DPS_512: f32 = 64.0_f32;
const FILTER_DT_FALLBACK_S: f32 = 0.02_f32;
const FILTER_DT_MIN_S: f32 = 0.001_f32;
const FILTER_DT_MAX_S: f32 = 0.1_f32;
const ACCEL_TRUST_MIN_G: f32 = 0.75_f32;
const ACCEL_TRUST_MAX_G: f32 = 1.25_f32;
const ACCEL_CORRECTION_GAIN_PER_S: f32 = 0.9_f32;

#[derive(Clone, Copy, Debug)]
pub struct ImuTiltComplementaryFilter {
    has_value: bool,
    gravity_sensor: [f32; 3],
    yaw_deg: f32,
}

impl Default for ImuTiltComplementaryFilter {
    fn default() -> Self {
        Self {
            has_value: false,
            gravity_sensor: [0.0, 0.0, 1.0],
            yaw_deg: 0.0,
        }
    }
}

impl ImuTiltComplementaryFilter {
    #[inline]
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn update_with_default_dt(&mut self, sample: &ImuRawSample) -> ImuTiltAngles {
        self.update(sample, FILTER_DT_FALLBACK_S)
    }

    #[inline]
    pub fn update(&mut self, sample: &ImuRawSample, dt_s: f32) -> ImuTiltAngles {
        let mut dt = dt_s;
        if !dt.is_finite() || dt <= 0.0 {
            dt = FILTER_DT_FALLBACK_S;
        }
        dt = dt.clamp(FILTER_DT_MIN_S, FILTER_DT_MAX_S);

        let accel = sample.accel_g_8g();
        let accel_mag = vec3_len(accel);
        let accel_unit = vec3_normalize(accel);

        if !self.has_value {
            self.gravity_sensor = accel_unit.unwrap_or([0.0, 0.0, 1.0]);
            self.has_value = true;
        }

        let [gx_dps, gy_dps, gz_dps] = sample.gyro_dps_512();
        let gyro_rad_s = [gx_dps * DEG_TO_RAD, gy_dps * DEG_TO_RAD, gz_dps * DEG_TO_RAD];

        // Gravity vector dynamics in sensor frame: g_dot = g x w.
        let gravity_dot = vec3_cross(self.gravity_sensor, gyro_rad_s);
        let mut predicted = vec3_add(self.gravity_sensor, vec3_scale(gravity_dot, dt));
        predicted = vec3_normalize(predicted).unwrap_or(self.gravity_sensor);

        if (ACCEL_TRUST_MIN_G..=ACCEL_TRUST_MAX_G).contains(&accel_mag) {
            if let Some(accel_unit) = accel_unit {
                // Low-pass correction toward accelerometer gravity direction.
                // Use direct vector blending (not cross-axis injection) to avoid
                // precession/limit-cycle behavior when the device is static.
                let alpha = (ACCEL_CORRECTION_GAIN_PER_S * dt).clamp(0.0, 1.0);
                let corrected = vec3_add(
                    vec3_scale(predicted, 1.0 - alpha),
                    vec3_scale(accel_unit, alpha),
                );
                predicted = vec3_normalize(corrected).unwrap_or(predicted);
            }
        }

        self.gravity_sensor = predicted;
        let yaw_rate_dps = gx_dps * self.gravity_sensor[0]
            + gy_dps * self.gravity_sensor[1]
            + gz_dps * self.gravity_sensor[2];
        self.yaw_deg = normalize_angle_deg(self.yaw_deg + yaw_rate_dps * dt);
        tilt_from_gravity_sensor(self.gravity_sensor, self.yaw_deg)
    }
}

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
        tilt_from_gravity_sensor([ax, ay, az], 0.0)
    }
}

#[inline]
fn tilt_from_gravity_sensor(gravity_sensor: [f32; 3], yaw_deg: f32) -> ImuTiltAngles {
    // Align pitch sign with the board's forward tilt convention used by raw sensor telemetry.
    let pitch_deg = (-gravity_sensor[0]).atan2(
        (gravity_sensor[1] * gravity_sensor[1] + gravity_sensor[2] * gravity_sensor[2]).sqrt(),
    ) * RAD_TO_DEG;
    let roll_deg = gravity_sensor[1].atan2(gravity_sensor[2]) * RAD_TO_DEG;
    ImuTiltAngles {
        pitch_deg,
        roll_deg,
        yaw_deg,
    }
}

#[inline]
fn normalize_angle_deg(angle_deg: f32) -> f32 {
    if !angle_deg.is_finite() {
        return 0.0;
    }
    let mut out = angle_deg;
    while out > 180.0 {
        out -= 360.0;
    }
    while out <= -180.0 {
        out += 360.0;
    }
    out
}

#[inline]
fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn vec3_scale(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

#[inline]
fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn vec3_len(v: [f32; 3]) -> f32 {
    vec3_dot(v, v).sqrt()
}

#[inline]
fn vec3_normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = vec3_len(v);
    if len <= 1.0e-6 {
        None
    } else {
        Some([v[0] / len, v[1] / len, v[2] / len])
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
