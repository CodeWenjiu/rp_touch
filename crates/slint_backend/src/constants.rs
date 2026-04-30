use co5300_driver::DISPLAY_WIDTH;

pub(crate) const STRIPE_H: usize = 48;
pub(crate) const STRIPE_WIDTH: usize = DISPLAY_WIDTH;
pub(crate) const STRIPE_PIXELS: usize = STRIPE_WIDTH * STRIPE_H;
pub(crate) const STRIPE_BUFFER_COUNT: usize = 3;
