use slint::{
    LogicalPosition,
    platform::{PointerEventButton, WindowEvent, software_renderer::MinimalSoftwareWindow},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TouchState {
    pub(crate) is_pressed: bool,
    pub(crate) last_x: u16,
    pub(crate) last_y: u16,
}

pub(crate) fn dispatch_touch_sample(
    window: &MinimalSoftwareWindow,
    state: &mut TouchState,
    sample: Option<(u16, u16)>,
    width: u16,
    height: u16,
) -> bool {
    match sample {
        Some((x, y)) => {
            let clamped_x = x.min(width.saturating_sub(1));
            let clamped_y = y.min(height.saturating_sub(1));
            let position = LogicalPosition::new(clamped_x as f32, clamped_y as f32);

            if state.is_pressed {
                if clamped_x != state.last_x || clamped_y != state.last_y {
                    window.dispatch_event(WindowEvent::PointerMoved { position });
                    state.last_x = clamped_x;
                    state.last_y = clamped_y;
                    return true;
                }
                return false;
            }

            window.dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            state.is_pressed = true;
            state.last_x = clamped_x;
            state.last_y = clamped_y;
            true
        }
        None => {
            if !state.is_pressed {
                return false;
            }

            let position = LogicalPosition::new(state.last_x as f32, state.last_y as f32);
            window.dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            state.is_pressed = false;
            true
        }
    }
}
