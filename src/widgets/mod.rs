pub mod rotary_knob;
pub mod waveform;

pub use rotary_knob::rotary_knob;
pub use waveform::{waveform, WaveformCache};

#[derive(Clone, Default)]
struct ValueEdit {
    text: String,
    editing: bool,
    focus: bool,
    invalid: bool,
    // Host value at submission and the snapped requested value.
    pending: Option<(f32, f32)>,
}

pub fn parameter_value<P: nice_plug::prelude::Param>(
    ui: &mut egui::Ui,
    id: egui::Id,
    param: &P,
    gui: &nice_plug::context::gui::GuiContext,
) {
    let mut state = ui.data(|d| d.get_temp::<ValueEdit>(id)).unwrap_or_default();
    if let Some((before, value)) = state.pending {
        let current = param.unmodulated_normalized_value();
        if current != before || current == value {
            state.pending = None;
        }
    }
    if state.editing {
        let response = ui.add(
            egui::TextEdit::singleline(&mut state.text)
                .id(id.with("input"))
                .desired_width(ui.available_width().min(110.0)),
        );
        if state.focus {
            response.request_focus();
            state.focus = false;
        }
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if escape || (response.lost_focus() && !enter) {
            state.editing = false;
            state.invalid = false;
        }
        if enter {
            if let Some(normalized) = param
                .string_to_normalized_value(&state.text)
                .filter(|v| v.is_finite())
            {
                let setter = gui.param_setter();
                let snapped = param.preview_normalized(param.preview_plain(normalized));
                setter.begin_set_parameter(param);
                setter.set_parameter(param, param.preview_plain(snapped));
                setter.end_set_parameter(param);
                state.pending = Some((param.unmodulated_normalized_value(), snapped));
                state.editing = false;
                state.invalid = false;
            } else {
                state.invalid = true;
                response.request_focus();
            }
        }
        if state.invalid {
            response.on_hover_text("Invalid value — Enter to retry, Esc to cancel");
        }
    } else {
        let normalized = state
            .pending
            .map(|(_, value)| value)
            .unwrap_or_else(|| param.unmodulated_normalized_value());
        if ui
            .add(
                egui::Label::new(param.normalized_value_to_string(normalized, true))
                    .sense(egui::Sense::click()),
            )
            .on_hover_text("Click to type a value. Enter commits; Esc cancels.")
            .clicked()
        {
            state.text = param.normalized_value_to_string(normalized, false);
            state.editing = true;
            state.focus = true;
            state.invalid = false;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, state));
}

/// Keep unsnapped drag progress locally: integer steps and delayed host echoes
/// must not discard small movements. Only the host setter updates the parameter.
fn parameter_drag<P: nice_plug::prelude::Param>(
    ui: &egui::Ui,
    response: &egui::Response,
    param: &P,
    gui: &nice_plug::context::gui::GuiContext,
) {
    let setter = gui.param_setter();
    let drag_id = response.id.with("normalized-drag");
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, param.default_plain_value());
        setter.end_set_parameter(param);
    } else {
        if response.drag_started() {
            ui.data_mut(|data| data.insert_temp(drag_id, param.unmodulated_normalized_value()));
            setter.begin_set_parameter(param);
        }
        if response.dragged() {
            let delta = ui.input(|i| {
                -i.pointer.delta().y * 0.005 * if i.modifiers.shift { 0.1 } else { 1.0 }
            });
            let value = ui.data_mut(|data| {
                let value = (data
                    .get_temp::<f32>(drag_id)
                    .unwrap_or_else(|| param.unmodulated_normalized_value())
                    + delta)
                    .clamp(0.0, 1.0);
                data.insert_temp(drag_id, value);
                value
            });
            setter.set_parameter(param, param.preview_plain(value));
        }
        if response.drag_stopped() {
            ui.data_mut(|data| data.remove::<f32>(drag_id));
            setter.end_set_parameter(param);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::{
        context::{
            gui::{GuiContext, GuiContextInner},
            PluginApi,
        },
        prelude::*,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub(super) struct Host(pub(super) Mutex<Vec<Option<f32>>>);
    impl GuiContextInner for Host {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Vst3
        }
        unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {
            self.0.lock().unwrap().push(None);
        }
        unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, value: f32) {
            // Deliberately do not echo edits: processing may be delayed by the host.
            self.0.lock().unwrap().push(Some(value));
        }
        unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {
            self.0.lock().unwrap().push(None);
        }
        fn get_state(&self) -> PluginState {
            unreachable!()
        }
        fn set_state(&self, _: PluginState) {
            unreachable!()
        }
        fn request_restart(&self) {}
    }

    #[test]
    fn typed_value_commits_one_gesture_and_rejects_invalid_text() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "test".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/DejaVuSans.ttf"
            ))),
        );
        fonts
            .families
            .insert(egui::FontFamily::Proportional, vec!["test".into()]);
        ctx.set_fonts(fonts);
        let host = Arc::new(Host::default());
        let gui = GuiContext::new(host.clone());
        let param = FloatParam::new(
            "Value",
            32.0,
            FloatRange::Linear {
                min: 0.0,
                max: 127.0,
            },
        );
        let id = egui::Id::new("value-test");
        let frame = |events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(200.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    parameter_value(ui, id, &param, &gui);
                },
            )
            .drop_without_applying_deltas();
        };
        frame(vec![]);
        let at = egui::pos2(10.0, 10.0);
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        frame(vec![]);
        let select_all = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        };
        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        for invalid in ["invalid", "NaN"] {
            frame(vec![
                select_all.clone(),
                egui::Event::Text(invalid.into()),
                enter.clone(),
            ]);
            assert!(host.0.lock().unwrap().is_empty(), "invalid input {invalid}");
        }
        frame(vec![select_all, egui::Event::Text("93".into()), enter]);
        let events = host.0.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], None);
        assert_eq!(param.preview_plain(events[1].unwrap()), 93.0);
        assert_eq!(events[2], None);
        assert_eq!(param.value(), 32.0, "only the host changes DSP parameters");
    }

    #[test]
    fn fine_integer_drags_accumulate_without_host_echo() {
        let ctx = egui::Context::default();
        let host = Arc::new(Host::default());
        let gui = GuiContext::new(host.clone());
        let param = IntParam::new("Fine drag", 32, IntRange::Linear { min: 0, max: 127 });
        let frame = |mut events: Vec<egui::Event>| {
            events.insert(
                0,
                egui::Event::ModifiersChanged(egui::Modifiers {
                    shift: true,
                    ..Default::default()
                }),
            );
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 200.0),
                )),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                rotary_knob(ui, &param, &gui);
            })
            .drop_without_applying_deltas();
        };
        frame(vec![]);
        let at = egui::pos2(20.0, 40.0);
        frame(vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ]);
        for step in 1..=32 {
            frame(vec![egui::Event::PointerMoved(
                at - egui::vec2(0.0, step as f32),
            )]);
        }
        frame(vec![egui::Event::PointerButton {
            pos: at - egui::vec2(0.0, 32.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        }]);
        let events = host.0.lock().unwrap();
        assert_eq!(
            events.iter().filter(|v| v.is_none()).count(),
            2,
            "one begin/end pair"
        );
        assert_eq!(events.first(), Some(&None));
        assert_eq!(events.last(), Some(&None));
        // 32px at 0.0005/px ≈ two integer steps, even after drag slop.
        let last = events.iter().filter_map(|v| *v).next_back().unwrap();
        assert_eq!(param.preview_plain(last), 34);
        assert_eq!(param.value(), 32, "no direct DSP mutation");
    }
}
