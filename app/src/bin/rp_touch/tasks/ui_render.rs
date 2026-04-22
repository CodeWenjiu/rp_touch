use embassy_time::{Duration, Timer};

use crate::shared::{
    DISPLAY_FRAMEBUFFER, IMU_WATCH, TOUCH_WATCH, UI_DATA_REFRESH_MS, UI_RENDER_PERIOD_MS,
};
use crate::slint_ui;

fn touch_sample_to_xy(sample: ft3168_driver::TouchSample) -> Option<(u16, u16)> {
    sample.map(|p| (p.x, p.y))
}

#[embassy_executor::task]
pub async fn ui_render_task() -> ! {
    let mut display = co5300_driver::Co5300::new_default();
    display.init_default().await;
    let framebuffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER) };
    framebuffer.fill_rgb565(0x0000);

    let mut backend = slint_backend::SlintBackend::init_default().ok();
    let ui = if backend.is_some() {
        slint_ui::create_app_ui().ok()
    } else {
        None
    };

    let ui_enabled = ui.is_some() && backend.is_some();
    if ui_enabled {
        if let (Some(ui_ref), Some(backend_ref)) = (ui.as_ref(), backend.as_mut()) {
            ui_ref.set_tilt_ratio(0.5);
            backend_ref.request_redraw();
            let _ = backend_ref.render_if_needed(&mut display);
        }
    } else {
        framebuffer.fill_rgb565(0x001F);
        display.write_framebuffer(framebuffer).await;
    }

    let mut imu_ui_rx = IMU_WATCH.receiver().unwrap();
    let mut touch_ui_rx = TOUCH_WATCH.receiver().unwrap();
    let mut latest_imu = imu_ui_rx.try_get().unwrap_or_default();
    let mut ui_dirty = true;
    let mut ui_data_ticks = 0u32;

    loop {
        while let Some(frame) = imu_ui_rx.try_changed() {
            latest_imu = frame;
            ui_dirty = true;
        }
        while let Some(frame) = touch_ui_rx.try_changed() {
            if let Some(backend_ref) = backend.as_mut() {
                let _ = backend_ref.inject_touch_sample(touch_sample_to_xy(frame.sample));
            }
        }

        ui_data_ticks = ui_data_ticks.saturating_add(UI_RENDER_PERIOD_MS as u32);
        if ui_data_ticks >= UI_DATA_REFRESH_MS as u32 {
            ui_data_ticks = 0;
            ui_dirty = true;
        }

        if ui_dirty {
            if let Some(ui_ref) = ui.as_ref() {
                let tilt = latest_imu.sample.tilt_deg_from_accel_8g();
                let pitch = tilt.pitch_deg;
                let ratio = ((pitch + 90.0) / 180.0).clamp(0.0, 1.0);
                ui_ref.set_tilt_ratio(ratio);
            }
            if let Some(backend_ref) = backend.as_ref() {
                backend_ref.request_redraw();
            }
            ui_dirty = false;
        }

        if ui_enabled {
            if let Some(backend_ref) = backend.as_mut() {
                let _ = backend_ref.render_if_needed(&mut display);
            }
        }

        Timer::after(Duration::from_millis(UI_RENDER_PERIOD_MS)).await;
    }
}
