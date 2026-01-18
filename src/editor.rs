use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::UltrawaveParams;

#[derive(Lens)]
struct EditorData {
    params: Arc<UltrawaveParams>,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (650, 500))
}

pub fn create(
    params: Arc<UltrawaveParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        cx.add_stylesheet(include_str!("theme.css"))
            .expect("Failed to load stylesheet");

        EditorData {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            Label::new(cx, "ULTRAWAVE")
                .font_family(vec![FamilyOwned::SansSerif])
                .font_size(28.0)
                .top(Pixels(15.0))
                .bottom(Pixels(25.0))
                .height(Pixels(40.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .class("title");

            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "PITCH").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.pitch).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "DECAY").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.dec).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "VOL").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.vol).class("knob");
                    })
                    .class("knob-container");
                })
                .class("knob-row");

                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "FILT F").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.fltf).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "FILT Q").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.fltq).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "SRR").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.srr).class("knob");
                    })
                    .class("knob-container");
                })
                .class("knob-row");
            })
            .class("knob-grid");
        })
        .class("main-container");
    })
}
