use egui::{pos2, Color32, Rect, Stroke, StrokeKind, Ui};

const DISPLAY_FULL_SCALE_12BIT: f32 = 2048.0;
const DISPLAY_DB_FLOOR: f32 = -60.0;

pub fn waveform(ui: &mut Ui, rect: Rect, samples: &[i16], markers: Option<(f32, f32)>) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::BLACK);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Inside,
    );
    let center = rect.center().y;
    painter.line_segment(
        [pos2(rect.left(), center), pos2(rect.right(), center)],
        Stroke::new(1.0, Color32::WHITE),
    );
    let width = rect.width().max(1.0) as usize;
    if !samples.is_empty() {
        for x in 0..width {
            let start = x * samples.len() / width;
            let end = ((x + 1) * samples.len() / width)
                .max(start + 1)
                .min(samples.len());
            let (mut low, mut high) = (0.0_f32, 0.0_f32);
            for sample in &samples[start..end] {
                let value = (*sample as f32 / DISPLAY_FULL_SCALE_12BIT).clamp(-1.0, 1.0);
                let scaled = if value == 0.0 {
                    0.0
                } else {
                    value.signum()
                        * ((20.0 * value.abs().log10()).clamp(DISPLAY_DB_FLOOR, 0.0)
                            - DISPLAY_DB_FLOOR)
                        / -DISPLAY_DB_FLOOR
                };
                low = low.min(scaled);
                high = high.max(scaled);
            }
            let px = rect.left() + x as f32;
            let amplitude = rect.height() * 0.5 - 5.0;
            painter.line_segment(
                [
                    pos2(px, center + low * amplitude),
                    pos2(px, center + high * amplitude),
                ],
                Stroke::new(1.0, Color32::WHITE),
            );
        }
    }
    if let Some((start, end)) = markers {
        for (position, left) in [(start, true), (end, false)] {
            let x = rect.left() + rect.width() * position.clamp(0.0, 1.0);
            painter.line_segment(
                [pos2(x, rect.top() + 3.0), pos2(x, rect.bottom() - 3.0)],
                Stroke::new(2.0, Color32::WHITE),
            );
            let bracket = if left {
                Rect::from_min_max(
                    pos2(x - 4.0, rect.top() + 7.0),
                    pos2(x, rect.bottom() - 7.0),
                )
            } else {
                Rect::from_min_max(
                    pos2(x, rect.top() + 7.0),
                    pos2(x + 4.0, rect.bottom() - 7.0),
                )
            };
            painter.rect_filled(bracket, 0.0, Color32::WHITE);
        }
    }
}
