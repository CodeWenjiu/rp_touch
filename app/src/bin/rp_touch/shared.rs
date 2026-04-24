use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};

pub(crate) const IMU_REPORT_PERIOD_MS: u64 = 500;
pub(crate) const SENSOR_WATCH_PERIOD_MS: u64 = 5;
pub(crate) const CHIP_TEMP_SAMPLE_PERIOD_MS: u64 = 800;

pub(crate) static IMU_WATCH: Watch<CriticalSectionRawMutex, qmi8658_driver::ImuFrame, 4> =
    Watch::new();
pub(crate) static TOUCH_WATCH: Watch<CriticalSectionRawMutex, ft3168_driver::TouchFrame, 4> =
    Watch::new();
pub(crate) static CHIP_TEMP_WATCH: Watch<CriticalSectionRawMutex, i32, 4> = Watch::new();
pub(crate) static UI_STATE_WATCH: Watch<CriticalSectionRawMutex, UiRenderState, 4> = Watch::new();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum UiRenderState {
    #[default]
    Starting,
    Running,
    InitFailedBackend,
    InitFailedUi,
}
