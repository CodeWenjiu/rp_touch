use embassy_time::{Duration, Instant};

use crate::shared::{IMU_WATCH, TOUCH_WATCH, UI_STATE_WATCH, UiRenderState};
use crate::slint_ui;

const TILT_REDRAW_EPS: f32 = 0.002;
const IMU_REDRAW_FORCE_MS: u64 = 120;
const REDRAW_KEEPALIVE_MS: u64 = 250;

fn touch_sample_to_xy(sample: ft3168_driver::TouchSample) -> Option<(u16, u16)> {
    sample.map(|p| (p.x, p.y))
}

#[embassy_executor::task]
pub async fn ui_render_task() {
    let ui_state_tx = UI_STATE_WATCH.sender();
    ui_state_tx.send(UiRenderState::Starting);

    let mut display = co5300_driver::Co5300::new_default();
    display.init_default().await;

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
    let mut last_ratio = f32::NAN;
    let mut last_imu_redraw_at = Instant::now();
    let mut touch_active = false;
    let mut touch_x = 0i32;
    let mut touch_y = 0i32;
    let mut fps_window_start = Instant::now();
    let mut rendered_frame_count = 0u32;
    let mut last_rendered_at = Instant::now();

    loop {
        let mut imu_updated = false;
        if let Some(frame) = imu_ui_rx.try_changed() {
            latest_imu = frame;
            imu_updated = true;
        }

        if let Some(frame) = touch_ui_rx.try_changed() {
            let _ = backend.inject_touch_sample(touch_sample_to_xy(frame.sample));
            match frame.sample {
                Some(point) => {
                    touch_active = true;
                    touch_x = point.x as i32;
                    touch_y = point.y as i32;
                }
                None => {
                    touch_active = false;
                }
            }
        }

        ui.set_ui_touch_active(touch_active);
        if touch_active {
            ui.set_ui_touch_x(touch_x);
            ui.set_ui_touch_y(touch_y);
        }

        if imu_updated {
            let tilt = latest_imu.sample.tilt_deg_from_accel_8g();
            let pitch = tilt.pitch_deg;
            let ratio = ((pitch + 90.0) / 180.0).clamp(0.0, 1.0);
            let imu_force_due = Instant::now().saturating_duration_since(last_imu_redraw_at)
                >= Duration::from_millis(IMU_REDRAW_FORCE_MS);
            let imu_changed = last_ratio.is_nan() || (ratio - last_ratio).abs() >= TILT_REDRAW_EPS;
            if imu_changed || imu_force_due {
                ui.set_tilt_ratio(ratio);
                backend.request_redraw();
                last_ratio = ratio;
                last_imu_redraw_at = Instant::now();
            }
        }

        if backend.render_if_needed(&mut display) {
            rendered_frame_count = rendered_frame_count.saturating_add(1);
            last_rendered_at = Instant::now();
        } else if Instant::now().saturating_duration_since(last_rendered_at)
            >= Duration::from_millis(REDRAW_KEEPALIVE_MS)
        {
            backend.request_redraw();
            last_rendered_at = Instant::now();
        }

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(fps_window_start);
        if elapsed >= Duration::from_secs(1) {
            let elapsed_ms = elapsed.as_millis().max(1);
            let fps = ((rendered_frame_count as u64).saturating_mul(1000) / elapsed_ms) as i32;
            if ui.get_ui_fps() != fps {
                ui.set_ui_fps(fps);
                backend.request_redraw();
            }
            rendered_frame_count = 0;
            fps_window_start = now;
        }
    }
}
