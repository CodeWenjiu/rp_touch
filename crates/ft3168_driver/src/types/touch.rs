use crate::regs::FT3168_MAX_TOUCH_POINTS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchEvent {
    Down,
    Up,
    Contact,
    Reserved(u8),
}

impl TouchEvent {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Self::Down,
            1 => Self::Up,
            2 => Self::Contact,
            x => Self::Reserved(x),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TouchPoint {
    pub id: u8,
    pub event: Option<TouchEvent>,
    pub x: u16,
    pub y: u16,
    pub weight: u8,
    pub area: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TouchSample {
    pub touch_count: u8,
    pub points: [TouchPoint; FT3168_MAX_TOUCH_POINTS],
}

impl TouchSample {
    pub fn active_points(&self) -> &[TouchPoint] {
        &self.points[..self.touch_count.min(FT3168_MAX_TOUCH_POINTS as u8) as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TouchFrame {
    pub seq: u32,
    pub sample: TouchSample,
}
