use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::RamRecordParams;
use crate::widgets::{WaveformData, WaveformDisplay};

#[derive(Lens)]
struct EditorData {
    params: Arc<RamRecordParams>,
    waveform: WaveformData,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (700, 400))
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
            waveform: WaveformData::default(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            // Title bar
            Label::new(cx, "ULTRAWAVE RECORD")
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
                        Label::new(cx, "R1").class("buffer-label");
                        Label::new(cx, "READY").class("buffer-status");
                    })
                    .class("sidebar-section");
                })
                .class("sidebar");

                // Right main area (~80%)
                VStack::new(cx, |cx| {
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
                                ParamSlider::new(cx, EditorData::params, |p| &p.rec_len)
                                    .class("knob");
                            })
                            .class("knob-container");

                            VStack::new(cx, |cx| {
                                Label::new(cx, "RATE").class("knob-label");
                                ParamSlider::new(cx, EditorData::params, |p| &p.rec_rate)
                                    .class("knob");
                            })
                            .class("knob-container");
                        })
                        .class("knob-row");
                    })
                    .class("knob-grid");

                    // Waveform display with time indicator
                    HStack::new(cx, |cx| {
                        WaveformDisplay::new(cx, false, true).class("waveform-display");
                        Label::new(cx, "TIME: 0.0s / 10.0s").class("time-display");
                    })
                    .class("waveform-section");
                })
                .class("main-area");
            })
            .class("content-area");

            // Footer with breadcrumb and buffer tabs
            HStack::new(cx, |cx| {
                Label::new(cx, "RAM > R1 > REC").class("breadcrumb");

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
