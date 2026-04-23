use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};

pub(crate) const IMU_REPORT_PERIOD_MS: u64 = 500;
pub(crate) const SENSOR_WATCH_PERIOD_MS: u64 = 5;
pub(crate) const UI_RENDER_PERIOD_MS: u64 = 12;
pub(crate) const UI_DATA_REFRESH_MS: u64 = 12;

pub(crate) static mut DISPLAY_FRAMEBUFFER: co5300_driver::FrameBuffer =
    co5300_driver::FrameBuffer::new();

pub(crate) static IMU_WATCH: Watch<CriticalSectionRawMutex, qmi8658_driver::ImuFrame, 4> =
    Watch::new();
pub(crate) static TOUCH_WATCH: Watch<CriticalSectionRawMutex, ft3168_driver::TouchFrame, 4> =
    Watch::new();
pub(crate) static UI_STATE_WATCH: Watch<CriticalSectionRawMutex, UiRenderState, 4> = Watch::new();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum UiRenderState {
    #[default]
    Starting,
    Running,
    InitFailedBackend,
    InitFailedUi,
}
