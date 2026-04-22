use alloc::{boxed::Box, rc::Rc};
use core::time::Duration;

use embassy_time::Instant;
use slint::{
    PhysicalSize, PlatformError,
    platform::{Platform, SetPlatformError, software_renderer::MinimalSoftwareWindow},
};

pub(crate) fn init_window(
    width: u32,
    height: u32,
) -> Result<Rc<MinimalSoftwareWindow>, PlatformError> {
    let window = MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
    );
    let platform = EmbeddedPlatform {
        window: window.clone(),
        start_micros: Instant::now().as_micros(),
    };

    slint::platform::set_platform(Box::new(platform)).map_err(platform_error_from_set_platform)?;

    window.set_size(PhysicalSize::new(width, height));
    Ok(window)
}

struct EmbeddedPlatform {
    window: Rc<MinimalSoftwareWindow>,
    start_micros: u64,
}

impl Platform for EmbeddedPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        let now = Instant::now().as_micros();
        Duration::from_micros(now.saturating_sub(self.start_micros))
    }
}

fn platform_error_from_set_platform(error: SetPlatformError) -> PlatformError {
    match error {
        SetPlatformError::AlreadySet => "Slint platform was already initialized".into(),
        _ => "Slint platform initialization failed".into(),
    }
}
