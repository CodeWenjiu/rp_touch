use alloc::rc::Rc;

use co5300_driver::{Co5300, DISPLAY_HEIGHT, DISPLAY_WIDTH};
use slint::{PlatformError, platform::software_renderer::MinimalSoftwareWindow};

use crate::pipeline::new_pipeline;
use crate::platform::init_window;
use crate::touch::{TouchState, dispatch_touch_sample};

pub struct SlintBackend {
    window: Rc<MinimalSoftwareWindow>,
    touch_state: TouchState,
}

impl SlintBackend {
    pub fn init_default() -> Result<Self, PlatformError> {
        Self::init(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }

    pub fn init(width: u32, height: u32) -> Result<Self, PlatformError> {
        let window = init_window(width, height)?;
        Ok(Self {
            window,
            touch_state: TouchState::default(),
        })
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn inject_touch_sample(&mut self, sample: Option<(u16, u16)>) -> bool {
        let dispatched = dispatch_touch_sample(
            &self.window,
            &mut self.touch_state,
            sample,
            DISPLAY_WIDTH as u16,
            DISPLAY_HEIGHT as u16,
        );
        if dispatched {
            self.window.request_redraw();
        }
        dispatched
    }

    /// Render the current Slint frame if needed.
    ///
    /// At the top, drains any pending stripe transfers from the previous
    /// frame so that rendering and transfer pipeline across frames.
    /// Returns `true` if pixels were actually rendered.
    pub fn render_if_needed(&mut self, display: &mut Co5300<'static>) -> bool {
        // Drain previous frame's DMA transfers before rendering new frame.
        display.drain_pending_stripes();

        slint::platform::update_timers_and_animations();

        let mut rendered = false;
        let pipeline = new_pipeline(display);
        self.window.draw_if_needed(|renderer| {
            let dirty = renderer.render_by_line(pipeline);
            let bb = dirty.bounding_box_size();
            rendered = bb.width > 0 && bb.height > 0;
        });
        // pipeline dropped → commit() submits remaining stripes (non-blocking)

        rendered
    }
}
