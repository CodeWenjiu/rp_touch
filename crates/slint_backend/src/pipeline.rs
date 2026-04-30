use core::ops::Range;

use co5300_driver::{Co5300, DISPLAY_HEIGHT};
use embassy_time::{block_for, Duration};
use slint::platform::software_renderer::LineBufferProvider;

use crate::constants::{STRIPE_BUFFER_COUNT, STRIPE_H, STRIPE_PIXELS, STRIPE_WIDTH};
use crate::ui_pixel::UiPixel;

static mut STRIPE_BUFFERS: [[u16; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT] =
    [[0; STRIPE_PIXELS]; STRIPE_BUFFER_COUNT];

const MAX_LINE_SPANS: usize = 4;

#[derive(Clone, Copy)]
enum StripeJob {
    FullWidth {
        buf: usize,
        start_line: usize,
        rows: usize,
    },
    LineSpan {
        buf: usize,
        local_line: usize,
        abs_line: usize,
        x_start: usize,
        width: usize,
    },
}

impl StripeJob {
    fn buf(self) -> usize {
        match self {
            Self::FullWidth { buf, .. } => buf,
            Self::LineSpan { buf, .. } => buf,
        }
    }
}

#[derive(Clone, Copy)]
struct LineDirtySpans {
    count: u8,
    starts: [u16; MAX_LINE_SPANS],
    ends: [u16; MAX_LINE_SPANS],
}

impl LineDirtySpans {
    const EMPTY: Self = Self {
        count: 0,
        starts: [0; MAX_LINE_SPANS],
        ends: [0; MAX_LINE_SPANS],
    };

    fn add_span(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }

        let mut merged_start = start as u16;
        let mut merged_end = end as u16;
        let mut i = 0usize;

        while i < self.count as usize {
            let s = self.starts[i];
            let e = self.ends[i];
            if merged_start <= e && merged_end >= s {
                merged_start = merged_start.min(s);
                merged_end = merged_end.max(e);

                let count = self.count as usize;
                for j in (i + 1)..count {
                    self.starts[j - 1] = self.starts[j];
                    self.ends[j - 1] = self.ends[j];
                }
                self.count = self.count.saturating_sub(1);
                continue;
            }
            i += 1;
        }

        let count = self.count as usize;
        if count >= MAX_LINE_SPANS {
            self.count = 1;
            self.starts[0] = 0;
            self.ends[0] = STRIPE_WIDTH as u16;
            return;
        }

        let mut insert_at = 0usize;
        while insert_at < count && self.starts[insert_at] < merged_start {
            insert_at += 1;
        }

        for j in (insert_at..count).rev() {
            self.starts[j + 1] = self.starts[j];
            self.ends[j + 1] = self.ends[j];
        }
        self.starts[insert_at] = merged_start;
        self.ends[insert_at] = merged_end;
        self.count += 1;
    }

    fn is_full_width(&self) -> bool {
        self.count == 1 && self.starts[0] == 0 && self.ends[0] as usize == STRIPE_WIDTH
    }
}

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
    render_line_spans: [LineDirtySpans; STRIPE_H],
    render_active: bool,
    inflight_job: Option<StripeJob>,
    pending_job: Option<StripeJob>,
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
            render_line_spans: [LineDirtySpans::EMPTY; STRIPE_H],
            render_active: false,
            inflight_job: None,
            pending_job: None,
            buffer_cursor: 0,
        }
    }

    fn line_slice_mut(&mut self, buf: usize, line: usize) -> &mut [u16] {
        let start = line * STRIPE_WIDTH;
        let end = start + STRIPE_WIDTH;
        &mut self.buffers[buf][start..end]
    }

    fn is_buffer_reserved(&self, buf: usize) -> bool {
        self.inflight_job.map(StripeJob::buf) == Some(buf)
            || self.pending_job.map(StripeJob::buf) == Some(buf)
    }

    fn poll_transfer_progress(&mut self) {
        // Poll the display driver: if the current DMA transfer finished,
        // free the inflight slot so a pending job can start.
        if self.inflight_job.is_some() && self.display.poll_fullwidth_stripe_transfer_done() {
            self.inflight_job = None;
        }

        if self.inflight_job.is_none() {
            if let Some(job) = self.pending_job.take() {
                self.start_transfer(job);
            }
        }
    }

    fn pick_render_buffer(&mut self) -> usize {
        loop {
            for offset in 0..STRIPE_BUFFER_COUNT {
                let idx = (self.buffer_cursor + offset) % STRIPE_BUFFER_COUNT;
                if !self.is_buffer_reserved(idx) {
                    self.buffer_cursor = (idx + 1) % STRIPE_BUFFER_COUNT;
                    return idx;
                }
            }

            self.poll_transfer_progress();
            // Yield for ~1 µs instead of tight spin-loop.
            block_for(Duration::from_micros(1));
        }
    }

    fn start_new_stripe(&mut self, start_line: usize) {
        self.render_active = true;
        self.render_start_line = start_line;
        self.render_used_lines = 0;
        self.render_line_spans = [LineDirtySpans::EMPTY; STRIPE_H];
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
        local
    }

    fn current_stripe_is_full_width(&self) -> bool {
        if !self.render_active || self.render_used_lines == 0 {
            return false;
        }

        for local_line in 0..self.render_used_lines {
            if !self.render_line_spans[local_line].is_full_width() {
                return false;
            }
        }

        true
    }

    fn start_transfer(&mut self, job: StripeJob) {
        let (display, buffers) = (&mut self.display, &self.buffers);
        match job {
            StripeJob::FullWidth {
                buf,
                start_line,
                rows,
            } => {
                let pixels = &buffers[buf][..rows * STRIPE_WIDTH];
                display.begin_fullwidth_stripe_transfer(start_line, rows, pixels);
            }
            StripeJob::LineSpan {
                buf,
                local_line,
                abs_line,
                x_start,
                width,
            } => {
                let line_start = local_line * STRIPE_WIDTH;
                let span_start = line_start + x_start;
                let span_end = span_start + width;
                let pixels = &buffers[buf][span_start..span_end];
                display.begin_region_transfer(x_start, abs_line, width, 1, pixels);
            }
        }
        self.inflight_job = Some(job);
    }

    fn enqueue_job(&mut self, job: StripeJob) {
        loop {
            self.poll_transfer_progress();

            if self.inflight_job.is_none() {
                self.start_transfer(job);
                return;
            }

            if self.pending_job.is_none() {
                self.pending_job = Some(job);
                return;
            }

            block_for(Duration::from_micros(1));
        }
    }

    fn submit_current_stripe(&mut self) {
        if !self.render_active || self.render_used_lines == 0 {
            self.render_active = false;
            self.render_used_lines = 0;
            self.render_line_spans = [LineDirtySpans::EMPTY; STRIPE_H];
            return;
        }

        let full_width = self.current_stripe_is_full_width();
        let render_buf = self.render_buf;
        let render_start_line = self.render_start_line;
        let render_used_lines = self.render_used_lines;
        let render_line_spans = self.render_line_spans;

        self.render_active = false;
        self.render_used_lines = 0;
        self.render_line_spans = [LineDirtySpans::EMPTY; STRIPE_H];

        if full_width {
            self.enqueue_job(StripeJob::FullWidth {
                buf: render_buf,
                start_line: render_start_line,
                rows: render_used_lines,
            });
            return;
        }

        for local_line in 0..render_used_lines {
            let spans = render_line_spans[local_line];
            for i in 0..spans.count as usize {
                let x_start = spans.starts[i] as usize;
                let x_end = spans.ends[i] as usize;
                if x_start >= x_end {
                    continue;
                }

                self.enqueue_job(StripeJob::LineSpan {
                    buf: render_buf,
                    local_line,
                    abs_line: render_start_line + local_line,
                    x_start,
                    width: x_end - x_start,
                });
            }
        }
    }

    /// Submit the current stripe and drain pending jobs.
    /// Called on Drop; uses `block_for` instead of tight spin-loop.
    fn commit(&mut self) {
        self.submit_current_stripe();

        while self.inflight_job.is_some() || self.pending_job.is_some() {
            self.poll_transfer_progress();
            if self.inflight_job.is_some() || self.pending_job.is_some() {
                block_for(Duration::from_micros(2));
            }
        }
    }
}

impl Drop for StripePipeline<'_> {
    fn drop(&mut self) {
        self.commit();
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

        let words = &mut self.line_slice_mut(self.render_buf, local_line)[render_range.clone()];
        let pixels = unsafe {
            core::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut UiPixel, words.len())
        };
        render_fn(pixels);
        self.render_line_spans[local_line].add_span(render_range.start, render_range.end);
    }
}
