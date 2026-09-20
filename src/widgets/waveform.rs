use egui::{pos2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};
use nice_plug::{
    context::gui::GuiContext,
    prelude::{IntParam, Param},
};

const DISPLAY_FULL_SCALE_12BIT: f32 = 2048.0;
const DISPLAY_DB_FLOOR: f32 = -60.0;

#[derive(Default)]
pub struct WaveformCache {
    key: Option<(usize, usize, usize, u64, usize)>,
    peaks: Vec<(i16, i16)>,
}

#[derive(Clone)]
struct MarkerEdit {
    handle: usize,
    before: f32,
    pending: f32,
    dragging: bool,
}

impl WaveformCache {
    fn update(&mut self, identity: (usize, usize, usize, u64), width: usize, samples: &[i16]) {
        let key = (identity.0, identity.1, identity.2, identity.3, width);
        if self.key == Some(key) && !samples.is_empty() {
            return;
        }
        self.peaks.clear();
        self.peaks.reserve(width);
        for x in 0..width {
            let start = x * samples.len() / width;
            let end = ((x + 1) * samples.len() / width)
                .max(start + 1)
                .min(samples.len());
            let mut peak = (0, 0);
            for &sample in samples.get(start..end).unwrap_or_default() {
                peak.0 = peak.0.min(sample);
                peak.1 = peak.1.max(sample);
            }
            self.peaks.push(peak);
        }
        // An unavailable read must not poison a later retry at the same revision.
        self.key = (!samples.is_empty()).then_some(key);
    }
}

fn display_scale(sample: i16) -> f32 {
    let value = (sample as f32 / DISPLAY_FULL_SCALE_12BIT).clamp(-1.0, 1.0);
    if value == 0.0 {
        return 0.0;
    }
    value.signum() * ((20.0 * value.abs().log10()).clamp(DISPLAY_DB_FLOOR, 0.0) - DISPLAY_DB_FLOOR)
        / -DISPLAY_DB_FLOOR
}

#[allow(clippy::too_many_arguments)]
pub fn waveform(
    ui: &mut Ui,
    rect: Rect,
    samples: &[i16],
    identity: (usize, usize, usize, u64),
    cache: &mut WaveformCache,
    markers: Option<(&IntParam, &IntParam)>,
    cursor: Option<f32>,
    gui: &GuiContext,
) {
    let response = ui.interact(rect, ui.id().with("waveform"), Sense::click_and_drag());
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
    cache.update(identity, width, samples);
    let amplitude = rect.height() * 0.5 - 5.0;
    for (x, &(low, high)) in cache.peaks.iter().enumerate() {
        let px = rect.left() + x as f32;
        painter.line_segment(
            [
                pos2(px, center + display_scale(low) * amplitude),
                pos2(px, center + display_scale(high) * amplitude),
            ],
            Stroke::new(1.0, Color32::WHITE),
        );
    }

    if let Some((start, end)) = markers {
        let mut values = [
            start.unmodulated_normalized_value(),
            end.unmodulated_normalized_value(),
        ];
        let drag_id = response.id.with("marker");
        let mut edit = ui.data(|d| d.get_temp::<MarkerEdit>(drag_id));
        if let Some(state) = &edit {
            if !state.dragging
                && (values[state.handle] != state.before || values[state.handle] == state.pending)
            {
                edit = None;
                ui.data_mut(|d| d.remove::<MarkerEdit>(drag_id));
            } else {
                values[state.handle] = state.pending;
            }
        }
        if response.drag_started() {
            let pointer = ui
                .input(|i| i.pointer.press_origin())
                .unwrap_or(rect.center());
            let target = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let distances = [(target - values[0]).abs(), (target - values[1]).abs()];
            // At equality split the hit area around the coincident handles.
            let handle = if distances[0] < distances[1]
                || (distances[0] == distances[1] && target <= values[0])
            {
                0usize
            } else {
                1
            };
            edit = Some(MarkerEdit {
                handle,
                before: values[handle],
                pending: values[handle],
                dragging: true,
            });
            gui.param_setter().begin_set_parameter([start, end][handle]);
        }
        if response.dragged() {
            if let (Some(pointer), Some(state)) = (response.interact_pointer_pos(), edit.as_mut()) {
                let normalized = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let param = [start, end][state.handle];
                state.pending = param.preview_normalized(param.preview_plain(normalized));
                values[state.handle] = state.pending;
                gui.param_setter()
                    .set_parameter(param, param.preview_plain(normalized));
            }
        }
        if response.drag_stopped() {
            if let Some(state) = edit.as_mut() {
                gui.param_setter()
                    .end_set_parameter([start, end][state.handle]);
                state.dragging = false;
            }
        }
        if let Some(state) = edit {
            ui.data_mut(|d| d.insert_temp(drag_id, state));
        }
        for (index, position) in values.into_iter().enumerate() {
            let x = rect.left() + rect.width() * position;
            painter.line_segment(
                [pos2(x, rect.top() + 3.0), pos2(x, rect.bottom() - 3.0)],
                Stroke::new(2.0, Color32::WHITE),
            );
            let bracket = if index == 0 {
                Rect::from_min_max(
                    pos2(x, rect.top() + 7.0),
                    pos2(x + 4.0, rect.bottom() - 7.0),
                )
            } else {
                Rect::from_min_max(
                    pos2(x - 4.0, rect.top() + 7.0),
                    pos2(x, rect.bottom() - 7.0),
                )
            };
            painter.rect_filled(bracket, 0.0, Color32::WHITE);
        }
    }
    if let Some(position) = cursor {
        let x = rect.left() + rect.width() * position.clamp(0.0, 1.0);
        painter.line_segment(
            [pos2(x, rect.top() + 1.0), pos2(x, rect.bottom() - 1.0)],
            Stroke::new(3.0, Color32::BLACK),
        );
        painter.line_segment(
            [pos2(x, rect.top() + 1.0), pos2(x, rect.bottom() - 1.0)],
            Stroke::new(1.0, Color32::WHITE),
        );
        painter.rect_filled(
            Rect::from_center_size(pos2(x, rect.top() + 3.0), egui::vec2(7.0, 4.0)),
            0.0,
            Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn marker_drag_snaps_crosses_and_yields_to_host_updates() {
        let ctx = egui::Context::default();
        let host = Arc::new(crate::widgets::tests::Host::default());
        let gui = GuiContext::new(host.clone());
        let mut cache = WaveformCache::default();
        let param = |value| {
            IntParam::new(
                "Marker",
                value,
                nice_plug::prelude::IntRange::Linear { min: 0, max: 127 },
            )
        };
        let start = param(25);
        let end = param(25);
        let mut frame = |events, end: &IntParam| {
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    waveform(
                        ui,
                        Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(400.0, 60.0)),
                        &[],
                        (0, 0, 0, 0),
                        &mut cache,
                        Some((&start, end)),
                        None,
                        &gui,
                    );
                },
            );
            let markers: Vec<f32> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke } if stroke.width == 2.0 => {
                        Some(points[0].x)
                    }
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            markers
        };
        frame(vec![], &end);
        let at = pos2(82.0, 30.0); // right of coincident handles at 78.74, not the panel midpoint
        frame(
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &end,
        );
        let target = pos2(12.3, 30.0); // END crosses STRT; integer value 4, not unsnapped 3.905
        frame(vec![egui::Event::PointerMoved(target)], &end);
        let drawn = frame(
            vec![egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &end,
        );
        assert!((drawn[0] - 400.0 * 25.0 / 127.0).abs() < 0.001);
        assert!((drawn[1] - 400.0 * 4.0 / 127.0).abs() < 0.001);
        let events = host.0.lock().unwrap();
        assert_eq!(events.iter().filter(|e| e.is_none()).count(), 2);
        assert_eq!(
            end.preview_plain(events.iter().filter_map(|v| *v).next_back().unwrap()),
            4
        );
        drop(events);
        // A host can clamp/automate before echoing our exact request.
        let drawn = frame(vec![], &param(90));
        assert!((drawn[1] - 400.0 * 90.0 / 127.0).abs() < 0.001);
    }

    #[test]
    fn cache_key_includes_registry_identity_and_reduces_before_scaling() {
        let mut cache = WaveformCache::default();
        cache.update((1, 0, 4, 2), 2, &[-10, 20, -30, 40]);
        assert_eq!(cache.peaks, [(-10, 20), (-30, 40)]);
        cache.update((2, 0, 4, 2), 2, &[1, 2, 3, 4]);
        assert_eq!(cache.peaks, [(0, 2), (0, 4)]);
        cache.update((2, 0, 4, 2), 2, &[]);
        assert_eq!(cache.peaks, [(0, 0), (0, 0)]);
        cache.update((2, 0, 4, 2), 2, &[-3, 8, -5, 9]);
        assert_eq!(cache.peaks, [(-3, 8), (-5, 9)]);
    }
}
