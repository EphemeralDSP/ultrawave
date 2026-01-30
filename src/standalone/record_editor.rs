use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::RamRecordParams;

#[derive(Lens)]
struct EditorData {
    params: Arc<RamRecordParams>,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (600, 350))
}

pub fn create(
    params: Arc<RamRecordParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        cx.add_stylesheet(include_str!("../theme.css"))
            .expect("Failed to load stylesheet");

        EditorData {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            // Title
            Label::new(cx, "ULTRAWAVE RECORD")
                .font_family(vec![FamilyOwned::SansSerif])
                .font_size(20.0)
                .class("title");

            // LED indicator
            HStack::new(cx, |cx| {
                Label::new(cx, "REC")
                    .font_family(vec![FamilyOwned::SansSerif])
                    .font_size(10.0)
                    .class("led-label");
                Element::new(cx).class("led-indicator");
            })
            .class("led-section");

            // 2×4 Knob Grid - Elektron Style
            VStack::new(cx, |cx| {
                // Top row: MLEV, MBAL, ILEV, IBAL
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "MLEV").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.mlev).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "MBAL").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.mbal).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "ILEV").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.ilev).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "IBAL").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.ibal).class("knob");
                    })
                    .class("knob-container");
                })
                .class("knob-row");

                // Bottom row: CUE1, CUE2, LEN, RATE
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "CUE1").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.cue1).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "CUE2").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.cue2).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "LEN").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.rec_len).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "RATE").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.rec_rate).class("knob");
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
