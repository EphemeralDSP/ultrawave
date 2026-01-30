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
    ViziaState::new(|| (850, 600))
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
            // Top section: Display + LED + Title + 2x4 Knob Grid
            HStack::new(cx, |cx| {
                // Left side: Display and LED
                VStack::new(cx, |cx| {
                    // Display section
                    VStack::new(cx, |cx| {
                        Label::new(cx, "100.0")
                            .font_family(vec![FamilyOwned::SansSerif])
                            .font_size(16.0)
                            .class("display-value");
                        Label::new(cx, "KIT:16")
                            .font_family(vec![FamilyOwned::SansSerif])
                            .font_size(10.0)
                            .class("display-label");
                        Label::new(cx, "RAM PLAY")
                            .font_family(vec![FamilyOwned::SansSerif])
                            .font_size(10.0)
                            .class("display-label");
                    })
                    .class("display-section");

                    // LED section
                    VStack::new(cx, |cx| {
                        Label::new(cx, "LED")
                            .font_family(vec![FamilyOwned::SansSerif])
                            .font_size(10.0)
                            .class("led-label");
                        Element::new(cx).class("led-indicator");
                    })
                    .class("led-section");
                })
                .class("left-panel");

                // Right side: 2x4 Knob Grid
                VStack::new(cx, |cx| {
                    // Top row: HLEV, HABL, ILEV, IBAL
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "HLEV").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.mlev)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "HABL").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.mbal)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "ILEV").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.ilev)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "IBAL").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.ibal)
                                .class("knob");
                        })
                        .class("knob-container");
                    })
                    .class("knob-row");

                    // Bottom row: CUE1, CUE2, LEN, RATE
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "CUE1").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.cue1)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "CUE2").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.cue2)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "LEN").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.rec_len)
                                .class("knob");
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RATE").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.record.rec_rate)
                                .class("knob");
                        })
                        .class("knob-container");
                    })
                    .class("knob-row");
                })
                .class("knob-grid");
            })
            .class("top-section");

            // Bottom section: Title + Play Parameters (smaller knobs)
            HStack::new(cx, |cx| {
                // Title on the left
                Label::new(cx, "ULTRAWAVE")
                    .font_family(vec![FamilyOwned::SansSerif])
                    .font_size(24.0)
                    .class("title");

                // Play parameters with smaller knobs (2 rows of 4)
                VStack::new(cx, |cx| {
                    // Top row: STRT, END, PTCH, HOLD
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "STRT").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.strt)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "END").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.end)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "PTCH").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.pitch)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "HOLD").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.hold)
                                .class("small-knob");
                        })
                        .class("small-knob-container");
                    })
                    .class("small-knob-row");

                    // Bottom row: DEC, RTRG, RTIM, SRR
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "DEC").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.dec)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTRG").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.rtrg)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTIM").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.rtim)
                                .class("small-knob");
                        })
                        .class("small-knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "SRR").class("small-knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.play.srr)
                                .class("small-knob");
                        })
                        .class("small-knob-container");
                    })
                    .class("small-knob-row");
                })
                .class("play-params-section");
            })
            .class("bottom-section");
        })
        .class("main-container");
    })
}
