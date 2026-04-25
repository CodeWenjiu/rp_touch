use embassy_time::{Duration, Instant};

use crate::shared::{CHIP_TEMP_WATCH, IMU_TEMP_WATCH, IMU_WATCH, TOUCH_WATCH, UI_STATE_WATCH, UiRenderState};
use crate::slint_ui;
use crate::SYSTEM_CLOCK_MHZ;

const TILT_REDRAW_EPS: f32 = 0.002;
const IMU_REDRAW_FORCE_MS: u64 = 120;
const REDRAW_KEEPALIVE_MS: u64 = 250;

fn touch_sample_to_xy(sample: ft3168_driver::TouchSample) -> Option<(u16, u16)> {
    sample.map(|p| (p.x, p.y))
}

fn round_to_i32(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
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
    ui.set_ui_cpu_mhz(SYSTEM_CLOCK_MHZ);
    backend.request_redraw();
    let _ = backend.render_if_needed(&mut display);

    let mut imu_ui_rx = IMU_WATCH.receiver().unwrap();
    let mut touch_ui_rx = TOUCH_WATCH.receiver().unwrap();
    let mut temp_ui_rx = CHIP_TEMP_WATCH.receiver().unwrap();
    let mut imu_temp_ui_rx = IMU_TEMP_WATCH.receiver().unwrap();
    let mut latest_imu = imu_ui_rx.try_get().unwrap_or_default();
    let mut latest_temp_c = temp_ui_rx.try_get().unwrap_or(0);
    let mut latest_imu_temp_c = imu_temp_ui_rx.try_get().unwrap_or(0);
    let mut last_ratio = f32::NAN;
    let mut last_imu_redraw_at = Instant::now();
    let mut touch_active = false;
    let mut touch_x = 0i32;
    let mut touch_y = 0i32;
    let mut fps_window_start = Instant::now();
    let mut rendered_frame_count = 0u32;
    let mut last_rendered_at = Instant::now();

    ui.set_ui_temp_c(latest_temp_c);
    ui.set_ui_imu_temp_c((latest_imu_temp_c + 5) / 10);

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

        if let Some(temp_c) = temp_ui_rx.try_changed() {
            latest_temp_c = temp_c;
            ui.set_ui_temp_c(latest_temp_c);
            backend.request_redraw();
        }

        if let Some(imu_temp_c) = imu_temp_ui_rx.try_changed() {
            latest_imu_temp_c = imu_temp_c;
            ui.set_ui_imu_temp_c((latest_imu_temp_c + 5) / 10);
            backend.request_redraw();
        }

        ui.set_ui_touch_active(touch_active);
        if touch_active {
            ui.set_ui_touch_x(touch_x);
            ui.set_ui_touch_y(touch_y);
        }

        if imu_updated {
            let tilt = latest_imu.sample.tilt_deg_from_accel_8g();
            let pitch = tilt.pitch_deg;
            let roll = tilt.roll_deg;
            let ratio = ((pitch + 90.0) / 180.0).clamp(0.0, 1.0);
            let imu_force_due = Instant::now().saturating_duration_since(last_imu_redraw_at)
                >= Duration::from_millis(IMU_REDRAW_FORCE_MS);
            let imu_changed = last_ratio.is_nan() || (ratio - last_ratio).abs() >= TILT_REDRAW_EPS;
            let pitch_i = round_to_i32(pitch);
            let roll_i = round_to_i32(roll);

            let mut imu_ui_changed = false;
            if ui.get_ui_pitch_deg() != pitch_i {
                ui.set_ui_pitch_deg(pitch_i);
                imu_ui_changed = true;
            }
            if ui.get_ui_roll_deg() != roll_i {
                ui.set_ui_roll_deg(roll_i);
                imu_ui_changed = true;
            }

            if imu_changed || imu_force_due {
                ui.set_tilt_ratio(ratio);
                imu_ui_changed = true;
                last_ratio = ratio;
                last_imu_redraw_at = Instant::now();
            }

            if imu_ui_changed {
                backend.request_redraw();
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
