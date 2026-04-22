slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppUi::new()?;
    ui.show()?;

    let weak = ui.as_weak();
    let start = std::time::Instant::now();
    let anim = slint::Timer::default();
    anim.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            if let Some(ui) = weak.upgrade() {
                let phase = start.elapsed().as_secs_f32();
                let ratio = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                ui.set_tilt_ratio(ratio);
            }
        },
    );

    slint::run_event_loop()
}
