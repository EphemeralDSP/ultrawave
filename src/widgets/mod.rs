pub mod rotary_knob;
pub mod vertical_slider;
pub mod waveform;

pub use rotary_knob::rotary_knob;
pub use vertical_slider::vertical_slider;
pub use waveform::waveform;

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
    struct Host(Mutex<Vec<Option<f32>>>);
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
    fn fine_integer_drags_accumulate_without_host_echo_for_both_widgets() {
        for slider in [false, true] {
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
                    if slider {
                        vertical_slider(ui, &param, &gui);
                    } else {
                        rotary_knob(ui, &param, &gui);
                    }
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
}
