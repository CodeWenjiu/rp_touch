#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

pub type TouchSample = Option<TouchPoint>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TouchFrame {
    pub seq: u32,
    pub sample: TouchSample,
}
