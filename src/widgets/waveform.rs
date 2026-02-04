use nih_plug_vizia::vizia::prelude::*;
use std::sync::Arc;

/// A waveform display widget that renders audio buffer data
/// with 1-bit monochrome Elektron-style aesthetic
pub struct WaveformDisplay {
    /// Audio samples to display (normalized -1.0 to 1.0)
    samples: Vec<f32>,
    /// Current time position for Record plugin (in seconds)
    current_time: f32,
    /// Maximum time (10 seconds)
    max_time: f32,
    /// Start position for Play plugin (0.0 to 1.0)
    start_pos: f32,
    /// End position for Play plugin (0.0 to 1.0)
    end_pos: f32,
    /// Whether to show start/end markers (Play plugin)
    show_markers: bool,
    /// Whether to show time display (Record plugin)
    show_time: bool,
}

impl WaveformDisplay {
    pub fn new(cx: &mut Context, show_markers: bool, show_time: bool) -> Handle<Self> {
        Self {
            samples: Vec::new(),
            current_time: 0.0,
            max_time: 10.0,
            start_pos: 0.0,
            end_pos: 1.0,
            show_markers,
            show_time,
        }
        .build(cx, |_| {})
    }

    /// Update the waveform data from i16 samples
    pub fn set_samples(&mut self, samples: &[i16]) {
        self.samples = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    }

    /// Set current recording time
    pub fn set_current_time(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.max_time);
    }

    /// Set start/end markers for playback
    pub fn set_markers(&mut self, start: f32, end: f32) {
        self.start_pos = start.clamp(0.0, 1.0);
        self.end_pos = end.clamp(0.0, 1.0);
    }
}

impl View for WaveformDisplay {
    fn element(&self) -> Option<&'static str> {
        Some("waveform-display")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.w;
        let h = bounds.h;

        // Background - dark
        let bg_paint = vg::Paint::color(vg::Color::rgb(10, 10, 10));
        let mut bg_path = vg::Path::new();
        bg_path.rect(x, y, w, h);
        canvas.fill_path(&bg_path, &bg_paint);

        // Border
        let border_paint = vg::Paint::color(vg::Color::rgb(64, 64, 64));
        border_paint.set_stroke_width(1.0);
        let mut border_path = vg::Path::new();
        border_path.rect(x, y, w, h);
        canvas.stroke_path(&border_path, &border_paint);

        // Waveform area dimensions (leave room for time display)
        let waveform_h = if self.show_time || self.show_markers {
            h - 20.0
        } else {
            h
        };
        let center_y = y + waveform_h / 2.0;
        let amplitude = waveform_h / 2.0 - 2.0;

        // Draw waveform
        if !self.samples.is_empty() {
            let waveform_paint = vg::Paint::color(vg::Color::rgb(0, 255, 204)); // Cyan
            waveform_paint.set_stroke_width(1.5);

            let mut waveform_path = vg::Path::new();

            // Downsample to fit width
            let samples_per_pixel = self.samples.len() as f32 / w;

            for px in 0..w as usize {
                let sample_start = (px as f32 * samples_per_pixel) as usize;
                let sample_end =
                    (((px + 1) as f32 * samples_per_pixel) as usize).min(self.samples.len());

                if sample_start < self.samples.len() {
                    // Find min/max in this pixel's range for better visualization
                    let (min_val, max_val) = self.samples[sample_start..sample_end]
                        .iter()
                        .fold((0.0f32, 0.0f32), |(min, max), &s| (min.min(s), max.max(s)));

                    let px_f = x + px as f32;
                    let y_min = center_y + min_val * amplitude;
                    let y_max = center_y + max_val * amplitude;

                    if px == 0 {
                        waveform_path.move_to(px_f, y_min);
                    } else {
                        waveform_path.line_to(px_f, y_min);
                    }
                    waveform_path.line_to(px_f, y_max);
                }
            }

            canvas.stroke_path(&waveform_path, &waveform_paint);
        } else {
            // Draw placeholder line when no data - subtle sine wave
            let placeholder_paint = vg::Paint::color(vg::Color::rgb(0, 255, 204));
            placeholder_paint.set_stroke_width(1.0);
            let mut placeholder_path = vg::Path::new();
            placeholder_path.move_to(x + 10.0, center_y);

            // Draw a simple sine-like placeholder
            for i in 0..w as usize - 20 {
                let px = x + 10.0 + i as f32;
                let py = center_y + (i as f32 * 0.1).sin() * 5.0;
                placeholder_path.line_to(px, py);
            }

            canvas.stroke_path(&placeholder_path, &placeholder_paint);
        }

        // Draw center line
        let center_paint = vg::Paint::color(vg::Color::rgb(40, 40, 40));
        center_paint.set_stroke_width(1.0);
        let mut center_path = vg::Path::new();
        center_path.move_to(x, center_y);
        center_path.line_to(x + w, center_y);
        canvas.stroke_path(&center_path, &center_paint);

        // Draw start/end markers for Play plugin
        if self.show_markers {
            let marker_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            marker_paint.set_stroke_width(2.0);

            // Start marker
            let start_x = x + self.start_pos * w;
            let mut start_path = vg::Path::new();
            start_path.move_to(start_x, y + 2.0);
            start_path.line_to(start_x, y + waveform_h - 2.0);
            canvas.stroke_path(&start_path, &marker_paint);

            // End marker
            let end_x = x + self.end_pos * w;
            let mut end_path = vg::Path::new();
            end_path.move_to(end_x, y + 2.0);
            end_path.line_to(end_x, y + waveform_h - 2.0);
            canvas.stroke_path(&end_path, &marker_paint);
        }

        // Draw time indicator for Record plugin
        if self.show_time {
            // Draw time indicator line at current position
            if self.current_time > 0.0 {
                let progress = self.current_time / self.max_time;
                let time_x = x + progress * w;
                let time_paint = vg::Paint::color(vg::Color::rgb(255, 255, 0));
                time_paint.set_stroke_width(2.0);
                let mut time_path = vg::Path::new();
                time_path.move_to(time_x, y + 2.0);
                time_path.line_to(time_x, y + waveform_h - 2.0);
                canvas.stroke_path(&time_path, &time_paint);
            }
        }
    }
}

/// Lens-compatible wrapper for waveform data
#[derive(Lens, Clone)]
pub struct WaveformData {
    pub samples: Arc<Vec<i16>>,
    pub current_time: f32,
    pub start_pos: f32,
    pub end_pos: f32,
}

impl Model for WaveformData {}

impl Default for WaveformData {
    fn default() -> Self {
        Self {
            samples: Arc::new(Vec::new()),
            current_time: 0.0,
            start_pos: 0.0,
            end_pos: 1.0,
        }
    }
}

/// Builder function for creating a waveform display with data binding
pub fn waveform_display<L: Lens<Target = WaveformData>>(
    cx: &mut Context,
    data: L,
    show_markers: bool,
    show_time: bool,
) -> Handle<WaveformDisplay> {
    WaveformDisplay::new(cx, show_markers, show_time).bind(data, |mut handle, data| {
        let data = data.get(&handle);
        if let Some(view) = handle.view.downcast_mut::<WaveformDisplay>() {
            view.set_samples(&data.samples);
            view.set_current_time(data.current_time);
            view.set_markers(data.start_pos, data.end_pos);
        }
    })
}
