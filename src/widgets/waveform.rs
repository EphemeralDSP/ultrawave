use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
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

const DISPLAY_FULL_SCALE_12BIT: f32 = 2048.0;
const DISPLAY_DB_FLOOR: f32 = -60.0;

impl WaveformDisplay {
    pub fn new(cx: &mut Context, show_markers: bool, show_time: bool) -> Handle<'_, Self> {
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
        self.samples = samples
            .iter()
            .map(|&s| {
                let normalized = (s as f32 / DISPLAY_FULL_SCALE_12BIT).clamp(-1.0, 1.0);
                Self::db_scaled_for_display(normalized)
            })
            .collect();
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

    fn db_scaled_for_display(sample: f32) -> f32 {
        let sign = sample.signum();
        let magnitude = sample.abs();
        if magnitude <= f32::EPSILON {
            return 0.0;
        }

        let db = 20.0 * magnitude.log10();
        let db_clamped = db.clamp(DISPLAY_DB_FLOOR, 0.0);
        let normalized = (db_clamped - DISPLAY_DB_FLOOR) / -DISPLAY_DB_FLOOR;
        sign * normalized
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

        // Background - black
        let bg_paint = vg::Paint::color(vg::Color::rgb(0, 0, 0));
        let mut bg_path = vg::Path::new();
        bg_path.rect(x, y, w, h);
        canvas.fill_path(&bg_path, &bg_paint);

        // Border - white
        let mut border_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        border_paint.set_line_width(1.0);
        let mut border_path = vg::Path::new();
        border_path.rect(x, y, w, h);
        canvas.stroke_path(&border_path, &border_paint);

        // Keep symmetric vertical padding so the waveform stays centered
        // inside its display box regardless of mode.
        let vertical_padding = if self.show_time || self.show_markers {
            4.0
        } else {
            2.0
        };
        let waveform_top = y + vertical_padding;
        let waveform_h = (h - vertical_padding * 2.0).max(4.0);
        let center_y = waveform_top + waveform_h / 2.0;
        let amplitude = waveform_h / 2.0 - 2.0;

        // Draw waveform - white for 1-bit monochrome
        if !self.samples.is_empty() {
            let mut waveform_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            waveform_paint.set_line_width(1.5);

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
        }

        // Draw center line - white for 1-bit monochrome
        let mut center_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        center_paint.set_line_width(1.0);
        let mut center_path = vg::Path::new();
        center_path.move_to(x, center_y);
        center_path.line_to(x + w, center_y);
        canvas.stroke_path(&center_path, &center_paint);

        // Draw start/end markers for Play plugin
        if self.show_markers {
            let mut marker_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            marker_paint.set_line_width(2.0);

            // Start marker position
            let start_x = x + self.start_pos * w;
            // End marker position
            let end_x = x + self.end_pos * w;

            // Draw vertical lines for start and end
            let mut start_path = vg::Path::new();
            start_path.move_to(start_x, waveform_top + 2.0);
            start_path.line_to(start_x, waveform_top + waveform_h - 2.0);
            canvas.stroke_path(&start_path, &marker_paint);

            let mut end_path = vg::Path::new();
            end_path.move_to(end_x, waveform_top + 2.0);
            end_path.line_to(end_x, waveform_top + waveform_h - 2.0);
            canvas.stroke_path(&end_path, &marker_paint);

            // Draw bracket indicators [▌ at start and ▐] at end
            // Using filled rectangles to create bracket shapes
            let bracket_width = 4.0;
            let bracket_height = waveform_h - 8.0;
            let bracket_y = waveform_top + 4.0;

            // Start bracket [▌ - left side
            let start_bracket_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            let mut start_bracket_path = vg::Path::new();
            start_bracket_path.rect(
                start_x - bracket_width,
                bracket_y,
                bracket_width,
                bracket_height,
            );
            canvas.fill_path(&start_bracket_path, &start_bracket_paint);

            // End bracket ▐] - right side
            let end_bracket_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            let mut end_bracket_path = vg::Path::new();
            end_bracket_path.rect(end_x, bracket_y, bracket_width, bracket_height);
            canvas.fill_path(&end_bracket_path, &end_bracket_paint);
        }

        // Draw time indicator for Record plugin
        if self.show_time {
            // Draw time indicator line at current position
            if self.current_time > 0.0 {
                let progress = self.current_time / self.max_time;
                let time_x = x + progress * w;
                let mut time_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
                time_paint.set_line_width(2.0);
                let mut time_path = vg::Path::new();
                time_path.move_to(time_x, waveform_top + 2.0);
                time_path.line_to(time_x, waveform_top + waveform_h - 2.0);
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

impl Data for WaveformData {
    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.samples, &other.samples)
            && self.current_time.to_bits() == other.current_time.to_bits()
            && self.start_pos.to_bits() == other.start_pos.to_bits()
            && self.end_pos.to_bits() == other.end_pos.to_bits()
    }
}

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
) -> Handle<'_, WaveformDisplay> {
    WaveformDisplay::new(cx, show_markers, show_time).bind(data, |mut handle, data| {
        let data = data.get(handle.context());
        handle.modify(|view: &mut WaveformDisplay| {
            view.set_samples(&data.samples);
            view.set_current_time(data.current_time);
            view.set_markers(data.start_pos, data.end_pos);
        });
    })
}
