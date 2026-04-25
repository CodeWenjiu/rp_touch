pub const DISPLAY_WIDTH: usize = 280;
pub const DISPLAY_HEIGHT: usize = 456;
pub const PIXEL_COUNT: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT;
pub const FRAMEBUFFER_BYTES: usize = PIXEL_COUNT * 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawError {
    OutOfBounds { x: usize, y: usize },
}

/// Full display frame buffer (280×456×2 ≈ 255 KiB).
///
/// # Stack safety
///
/// This struct is too large for typical embedded stacks (e.g. 16 KiB on RP2350).
/// Use one of these allocation strategies:
///
/// | Strategy               | Constructor        | Notes                                |
/// |------------------------|--------------------|--------------------------------------|
/// | Heap (global allocator)| `new_boxed()`      | Requires `alloc` feature.            |
/// | Static / BSS           | `static FB: FrameBuffer = ...` | Compile-time, zero-cost.     |
///
/// `FrameBuffer::new()` / `Default::default()` exists for static initialization
/// (`const` context) and for host-side simulators where stack space is ample.
/// Avoid calling `new()` or `default()` inside a function body on embedded targets.
#[derive(Clone)]
pub struct FrameBuffer {
    pixels: [u16; PIXEL_COUNT],
}

impl FrameBuffer {
    /// Creates a zero-filled `FrameBuffer`.
    ///
    /// # Stack warning
    ///
    /// On embedded targets (~255 KiB), calling this inside a function body will
    /// likely overflow the stack. Prefer [`Self::new_boxed`] (heap) or a `static`
    /// placement (BSS) instead.
    pub const fn new() -> Self {
        Self {
            pixels: [0; PIXEL_COUNT],
        }
    }

    /// Heap-allocates a zero-filled `FrameBuffer` via the global allocator.
    ///
    /// Requires the `alloc` crate feature.
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> alloc::boxed::Box<Self> {
        let mut boxed: alloc::boxed::Box<core::mem::MaybeUninit<Self>> =
            alloc::boxed::Box::new_uninit();
        unsafe {
            core::ptr::write_bytes(boxed.as_mut_ptr() as *mut u8, 0, core::mem::size_of::<Self>());
            boxed.assume_init()
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
