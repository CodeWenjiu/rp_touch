use embassy_time::{Duration, Timer};

use crate::shared::{
    DISPLAY_FRAMEBUFFER, IMU_WATCH, TOUCH_WATCH, UI_DATA_REFRESH_MS, UI_RENDER_PERIOD_MS,
    UI_STATE_WATCH, UiRenderState,
};
use crate::slint_ui;

fn touch_sample_to_xy(sample: ft3168_driver::TouchSample) -> Option<(u16, u16)> {
    sample.map(|p| (p.x, p.y))
}

#[embassy_executor::task]
pub async fn ui_render_task() {
    let ui_state_tx = UI_STATE_WATCH.sender();
    ui_state_tx.send(UiRenderState::Starting);

    let mut display = co5300_driver::Co5300::new_default();
    display.init_default().await;
    let framebuffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER) };
    framebuffer.fill_rgb565(0x0000);

    let mut backend = match slint_backend::SlintBackend::init_default() {
        Ok(v) => v,
        Err(_) => {
            ui_state_tx.send(UiRenderState::InitFailedBackend);
            return;
        }
    };
    let ui = match slint_ui::create_app_ui() {
        Ok(v) => v,
        Err(_) => {
            ui_state_tx.send(UiRenderState::InitFailedUi);
            return;
        }
    };
    ui_state_tx.send(UiRenderState::Running);

    ui.set_tilt_ratio(0.5);
    backend.request_redraw();
    let _ = backend.render_if_needed(&mut display);

    let mut imu_ui_rx = IMU_WATCH.receiver().unwrap();
    let mut touch_ui_rx = TOUCH_WATCH.receiver().unwrap();
    let mut latest_imu = imu_ui_rx.try_get().unwrap_or_default();
    let mut ui_dirty = true;
    let mut ui_data_ticks = 0u32;

    loop {
        if let Some(frame) = imu_ui_rx.try_changed() {
            latest_imu = frame;
            ui_dirty = true;
        }
        if let Some(frame) = touch_ui_rx.try_changed() {
            let _ = backend.inject_touch_sample(touch_sample_to_xy(frame.sample));
        }

        ui_data_ticks = ui_data_ticks.saturating_add(UI_RENDER_PERIOD_MS as u32);
        if ui_data_ticks >= UI_DATA_REFRESH_MS as u32 {
            ui_data_ticks = 0;
            ui_dirty = true;
        }

        if ui_dirty {
            let tilt = latest_imu.sample.tilt_deg_from_accel_8g();
            let pitch = tilt.pitch_deg;
            let ratio = ((pitch + 90.0) / 180.0).clamp(0.0, 1.0);
            ui.set_tilt_ratio(ratio);
            backend.request_redraw();
            ui_dirty = false;
        }

        let _ = backend.render_if_needed(&mut display);

        Timer::after(Duration::from_millis(UI_RENDER_PERIOD_MS)).await;
    }
}
