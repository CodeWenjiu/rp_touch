pub const DISPLAY_WIDTH: usize = 280;
pub const DISPLAY_HEIGHT: usize = 456;
pub const PIXEL_COUNT: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT;
pub const FRAMEBUFFER_BYTES: usize = PIXEL_COUNT * 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawError {
    OutOfBounds { x: usize, y: usize },
}

#[derive(Clone)]
pub struct FrameBuffer {
    pixels: [u16; PIXEL_COUNT],
}

impl FrameBuffer {
    pub const fn new() -> Self {
        Self {
            pixels: [0; PIXEL_COUNT],
        }
    }

    pub fn fill_rgb565(&mut self, color: u16) {
        self.pixels.fill(color.to_be());
    }

    pub fn set_pixel_rgb565(&mut self, x: usize, y: usize, color: u16) -> Result<(), DrawError> {
        if x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
            return Err(DrawError::OutOfBounds { x, y });
        }

        let idx = y * DISPLAY_WIDTH + x;
        self.pixels[idx] = color.to_be();
        Ok(())
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16] {
        &mut self.pixels
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, FRAMEBUFFER_BYTES) }
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}
