use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::RamPlayParams;

#[derive(Lens)]
struct EditorData {
    params: Arc<RamPlayParams>,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (600, 350))
}

pub fn create(
    params: Arc<RamPlayParams>,
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
            Label::new(cx, "ULTRAWAVE PLAY")
                .font_family(vec![FamilyOwned::SansSerif])
                .font_size(20.0)
                .class("title");

            // LED indicator
            HStack::new(cx, |cx| {
                Label::new(cx, "PLAY")
                    .font_family(vec![FamilyOwned::SansSerif])
                    .font_size(10.0)
                    .class("led-label");
                Element::new(cx).class("led-indicator");
            })
            .class("led-section");

            // 2×4 Knob Grid - Elektron Style
            VStack::new(cx, |cx| {
                // Top row: STRT, END, PTCH, HOLD
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "STRT").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.strt).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "END").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.end).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "PTCH").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.pitch).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "HOLD").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.hold).class("knob");
                    })
                    .class("knob-container");
                })
                .class("knob-row");

                // Bottom row: DEC, RTRG, RTIM, SRR
                HStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "DEC").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.dec).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "RTRG").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.rtrg).class("knob");
                    })
                    .class("knob-container");

                    VStack::new(cx, |cx| {
                        Label::new(cx, "RTIM").class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.rtim).class("knob");
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
