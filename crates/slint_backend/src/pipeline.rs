use core::ops::Range;

use co5300_driver::{Co5300, DISPLAY_HEIGHT};
use slint::platform::software_renderer::LineBufferProvider;

use crate::constants::{STRIPE_BUFFER_COUNT, STRIPE_H, STRIPE_PIXELS, STRIPE_WIDTH};
use crate::ui_pixel::UiPixel;

static mut STRIPE_BUFFERS: [[u16; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT] =
    [[0; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT];

pub(crate) fn new_pipeline<'a>(display: &'a mut Co5300<'static>) -> StripePipeline<'a> {
    let buffers = unsafe { &mut *core::ptr::addr_of_mut!(STRIPE_BUFFERS) };
    StripePipeline::new(display, buffers)
}

pub(crate) struct StripePipeline<'a> {
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
        if line >= DISPLAY_HEIGHT || range.start >= STRIPE_WIDTH || range.start >= range.end {
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
