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
    ViziaState::new(|| (700, 400))
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
            // Title bar
            Label::new(cx, "ULTRAWAVE PLAY")
                .font_family(vec![FamilyOwned::SansSerif])
                .font_size(18.0)
                .class("title");

            // Main content area with sidebar + grid
            HStack::new(cx, |cx| {
                // Left sidebar (~20%)
                VStack::new(cx, |cx| {
                    // Level meter
                    VStack::new(cx, |cx| {
                        Label::new(cx, "LEVEL").class("sidebar-label");
                        Element::new(cx).class("level-meter");
                    })
                    .class("sidebar-section");

                    // LEV knob
                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.lev).class("sidebar-knob");
                        Label::new(cx, "LEV").class("sidebar-knob-label");
                    })
                    .class("sidebar-knob-container");

                    // SRR knob
                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.srr).class("sidebar-knob");
                        Label::new(cx, "SRR").class("sidebar-knob-label");
                    })
                    .class("sidebar-knob-container");

                    // DIST knob
                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.dist).class("sidebar-knob");
                        Label::new(cx, "DIST").class("sidebar-knob-label");
                    })
                    .class("sidebar-knob-container");

                    // Buffer status
                    VStack::new(cx, |cx| {
                        Label::new(cx, "P01").class("buffer-label");
                        Label::new(cx, "SRC: R1").class("buffer-status");
                    })
                    .class("sidebar-section");
                })
                .class("sidebar");

                // Right main area (~80%)
                VStack::new(cx, |cx| {
                    // 2×4 Knob Grid - Elektron Style
                    VStack::new(cx, |cx| {
                        // Top row: PTCH, DEC, HOLD, BRR
                        HStack::new(cx, |cx| {
                            VStack::new(cx, |cx| {
                                Label::new(cx, "PTCH").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.pitch)
                                    .class("knob");
                            })
                            .class("knob-container");

                            VStack::new(cx, |cx| {
                                Label::new(cx, "DEC").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.dec).class("knob");
                            })
                            .class("knob-container");

                            VStack::new(cx, |cx| {
                                Label::new(cx, "HOLD").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.hold).class("knob");
                            })
                            .class("knob-container");

                            VStack::new(cx, |cx| {
                                Label::new(cx, "BRR").class("knob-label");
                                Element::new(cx).class("knob"); // Placeholder for BRR
                            })
                            .class("knob-container");
                        })
                        .class("knob-row");

                        // Bottom row: STRT, END, RTRG, RTIM
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
                                Label::new(cx, "RTRG").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.rtrg).class("knob");
                            })
                            .class("knob-container");

                            VStack::new(cx, |cx| {
                                Label::new(cx, "RTIM").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.rtim).class("knob");
                            })
                            .class("knob-container");
                        })
                        .class("knob-row");
                    })
                    .class("knob-grid");

                    // Waveform display placeholder
                    HStack::new(cx, |cx| {
                        Label::new(cx, "▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁").class("waveform-text");
                        Label::new(cx, "[▌    ▐] STRT/END").class("time-display");
                    })
                    .class("waveform-section");
                })
                .class("main-area");
            })
            .class("content-area");

            // Footer with breadcrumb and buffer tabs
            HStack::new(cx, |cx| {
                Label::new(cx, "RAM > R1 > PLAY").class("breadcrumb");

                HStack::new(cx, |cx| {
                    Label::new(cx, "[R1]").class("buffer-tab-active");
                    Label::new(cx, "[R2]").class("buffer-tab");
                    Label::new(cx, "[R3]").class("buffer-tab");
                    Label::new(cx, "[R4]").class("buffer-tab");
                })
                .class("buffer-tabs");
            })
            .class("footer");
        })
        .class("main-container");
    })
}
