#![no_std]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};
use core::ops::Range;
use core::time::Duration;

use co5300_driver::{Co5300, DISPLAY_HEIGHT, DISPLAY_WIDTH};
use embassy_time::Instant;
use slint::{
    PhysicalSize, PlatformError,
    platform::{
        Platform, SetPlatformError,
        software_renderer::{
            LineBufferProvider, MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
            Rgb565Pixel, TargetPixel,
        },
    },
};

const STRIPE_H: usize = 16;
const STRIPE_WIDTH: usize = DISPLAY_WIDTH;
const STRIPE_PIXELS: usize = STRIPE_WIDTH * STRIPE_H;
const STRIPE_BUFFER_COUNT: usize = 3;

static mut STRIPE_BUFFERS: [[u16; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT] =
    [[0; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT];

#[repr(transparent)]
#[derive(Clone, Copy, Default)]
struct UiPixel(u16);

impl TargetPixel for UiPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut native = Rgb565Pixel(u16::from_be(self.0));
        <Rgb565Pixel as TargetPixel>::blend(&mut native, color);
        self.0 = native.0.to_be();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let native = <Rgb565Pixel as TargetPixel>::from_rgb(r, g, b);
        Self(native.0.to_be())
    }
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

pub struct SlintBackend {
    window: Rc<MinimalSoftwareWindow>,
}

impl SlintBackend {
    pub fn init_default() -> Result<Self, PlatformError> {
        Self::init(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }

    pub fn init(width: u32, height: u32) -> Result<Self, PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        let platform = EmbeddedPlatform {
            window: window.clone(),
            start_micros: Instant::now().as_micros(),
        };

        slint::platform::set_platform(Box::new(platform))
            .map_err(platform_error_from_set_platform)?;

        window.set_size(PhysicalSize::new(width, height));

        Ok(Self { window })
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn render_if_needed(&mut self, display: &mut Co5300<'static>) -> bool {
        slint::platform::update_timers_and_animations();

        let mut rendered = false;
        let buffers = unsafe { &mut *core::ptr::addr_of_mut!(STRIPE_BUFFERS) };
        let pipeline = StripePipeline::new(display, buffers);

        self.window.draw_if_needed(|renderer| {
            renderer.render_by_line(pipeline);
            rendered = true;
        });

        rendered
    }
}

fn platform_error_from_set_platform(error: SetPlatformError) -> PlatformError {
    match error {
        SetPlatformError::AlreadySet => "Slint platform was already initialized".into(),
        _ => "Slint platform initialization failed".into(),
    }
}

struct StripePipeline<'a> {
    display: &'a mut Co5300<'static>,
    buffers: &'a mut [[u16; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT],
    render_buf: usize,
    render_start_line: usize,
    render_used_lines: usize,
    render_line_initialized: [bool; STRIPE_H],
    render_active: bool,
    inflight_buf: Option<usize>,
    buffer_cursor: usize,
}

impl<'a> StripePipeline<'a> {
    fn new(
        display: &'a mut Co5300<'static>,
        buffers: &'a mut [[u16; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT],
    ) -> Self {
        Self {
            display,
            buffers,
            render_buf: 0,
            render_start_line: 0,
            render_used_lines: 0,
            render_line_initialized: [false; STRIPE_H],
            render_active: false,
            inflight_buf: None,
            buffer_cursor: 0,
        }
    }

    fn line_slice_mut(&mut self, buf: usize, line: usize) -> &mut [u16] {
        let start = line * STRIPE_WIDTH;
        let end = start + STRIPE_WIDTH;
        &mut self.buffers[buf][start..end]
    }

    fn pick_render_buffer(&mut self) -> usize {
        for offset in 0..STRIPE_BUFFER_COUNT {
            let idx = (self.buffer_cursor + offset) % STRIPE_BUFFER_COUNT;
            if Some(idx) != self.inflight_buf {
                self.buffer_cursor = (idx + 1) % STRIPE_BUFFER_COUNT;
                return idx;
            }
        }

        if self.inflight_buf.is_some() {
            self.display.wait_fullwidth_stripe_transfer_done();
            self.inflight_buf = None;
        }
        self.buffer_cursor = 1 % STRIPE_BUFFER_COUNT;
        0
    }

    fn start_new_stripe(&mut self, start_line: usize) {
        self.render_active = true;
        self.render_start_line = start_line;
        self.render_used_lines = 0;
        self.render_line_initialized = [false; STRIPE_H];
        self.render_buf = self.pick_render_buffer();
    }

    fn ensure_render_line(&mut self, abs_line: usize) -> usize {
        if !self.render_active {
            self.start_new_stripe(abs_line);
        } else if abs_line < self.render_start_line || abs_line >= self.render_start_line + STRIPE_H
        {
            self.submit_current_stripe();
            self.start_new_stripe(abs_line);
        }

        let local = abs_line - self.render_start_line;
        if local + 1 > self.render_used_lines {
            self.render_used_lines = local + 1;
        }
        if !self.render_line_initialized[local] {
            self.line_slice_mut(self.render_buf, local).fill(0);
            self.render_line_initialized[local] = true;
        }

        local
    }

    fn submit_current_stripe(&mut self) {
        if !self.render_active || self.render_used_lines == 0 {
            self.render_active = false;
            self.render_used_lines = 0;
            return;
        }

        if self.inflight_buf.is_some() {
            self.display.wait_fullwidth_stripe_transfer_done();
            self.inflight_buf = None;
        }

        let rows = self.render_used_lines;
        let pixels = &self.buffers[self.render_buf][..rows * STRIPE_WIDTH];
        self.display
            .begin_fullwidth_stripe_transfer(self.render_start_line, rows, pixels);
        self.inflight_buf = Some(self.render_buf);

        self.render_active = false;
        self.render_used_lines = 0;
    }

    fn finalize(&mut self) {
        self.submit_current_stripe();
        if self.inflight_buf.is_some() {
            self.display.wait_fullwidth_stripe_transfer_done();
            self.inflight_buf = None;
        }
    }
}

impl Drop for StripePipeline<'_> {
    fn drop(&mut self) {
        self.finalize();
    }
}

impl LineBufferProvider for StripePipeline<'_> {
    type TargetPixel = UiPixel;

    fn process_line(
        &mut self,
        line: usize,
        range: Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        if range.start >= STRIPE_WIDTH || range.start >= range.end {
            return;
        }

        let end = core::cmp::min(range.end, STRIPE_WIDTH);
        let local_line = self.ensure_render_line(line);
        let render_range = range.start..end;

        let words = &mut self.line_slice_mut(self.render_buf, local_line)[render_range];
        let pixels = unsafe {
            core::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut UiPixel, words.len())
        };
        render_fn(pixels);
    }
}
