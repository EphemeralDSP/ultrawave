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

#[derive(Debug, Clone, Copy)]
struct BufferSelectEvent(i32);

pub fn create(
    params: Arc<RamRecordParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSans.ttf"));
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"));
        cx.set_default_font(&["DejaVu Sans", "SansSerif", "DejaVu Sans Mono"]);

        // Minimal CSS that works
        cx.add_stylesheet(
            r#"
            .main-container {
                background-color: #1a1a1a;
                width: 100%;
                height: 100%;
                child-space: 10px;
            }
            .title {
                color: #00ffcc;
                font-size: 18px;
                height: 30px;
            }
            .sidebar {
                width: 140px;
                background-color: #0d0d0d;
                child-space: 16px;
            }
            .knob-label {
                color: #00ffcc;
                font-size: 11px;
                text-align: center;
                margin-bottom: 4px;
            }
            .knob-container {
                width: 125px;
                height: 110px;
                child-space: 6px;
                background-color: #0a0a0a;
                border-radius: 8px;
                border: 1px solid #404040;
            }
            .knob {
                width: 60px;
                height: 60px;
                background-color: #1a1a1a;
                border-radius: 50%;
                border: 3px solid #505050;
            }
            .knob:hover {
                border-color: #00ffcc;
            }
            .knob .track {
                background-color: transparent;
            }
            .knob .fill {
                background-color: transparent;
            }
            .knob .thumb {
                background-color: #cccccc;
                width: 8px;
                height: 8px;
                border-radius: 50%;
                border: 1px solid #666666;
            }
            .knob .thumb:hover {
                background-color: #00cc99;
            }
            .main-area {
                background-color: #2a2a2a;
                width: Stretch(1.0);
                child-space: 15px;
            }
            .waveform-display {
                width: 100%;
                height: 40px;
                background-color: #0a0a0a;
            }
            .footer {
                height: 35px;
                background-color: #0d0d0d;
            }
        "#,
        )
        .expect("Failed to load stylesheet");

        EditorData {
            params: params.clone(),
            waveform: WaveformData::default(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            Label::new(cx, "ULTRAWAVE RECORD")
                .font_family(vec![FamilyOwned::Name("DejaVu Sans".to_string())])
                .font_size(18.0)
                .class("title");

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "LEV")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0)
                            .class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.lev)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0))
                            .class("knob");
                    })
                    .child_space(Stretch(0.5));

                    VStack::new(cx, |cx| {
                        Label::new(cx, "SRR")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0)
                            .class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.srr)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0))
                            .class("knob");
                    })
                    .child_space(Stretch(0.5));

                    VStack::new(cx, |cx| {
                        Label::new(cx, "DIST")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0)
                            .class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.dist)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0))
                            .class("knob");
                    })
                    .child_space(Stretch(0.5));

                    Label::new(cx, "LEVEL")
                        .color(Color::rgb(136, 136, 136))
                        .font_size(10.0);
                    Element::new(cx)
                        .width(Pixels(20.0))
                        .height(Pixels(80.0))
                        .background_color(Color::rgb(10, 10, 10));

                    Label::new(
                        cx,
                        EditorData::params.map(|p| match p.buffer.value() {
                            0 => "R1",
                            1 => "R2",
                            2 => "R3",
                            3 => "R4",
                            _ => "R1",
                        }),
                    )
                    .color(Color::rgb(255, 255, 255))
                    .font_size(14.0);
                    Label::new(cx, "READY")
                        .color(Color::rgb(0, 255, 204))
                        .font_size(9.0);
                })
                .class("sidebar");

                VStack::new(cx, |cx| {
                    // Knob grid - 2 rows x 4 knobs
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "MLEV").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.mlev).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.mlev)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "MBAL").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.mbal).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.mbal)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "ILEV").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.ilev).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.ilev)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "IBAL").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.ibal).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.ibal)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");
                    });

                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "CUE1").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.cue1).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.cue1)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "CUE2").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.cue2).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.cue2)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "LEN").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rec_len).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.rec_len)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RATE").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rec_rate).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.rec_rate)))
                                .color(Color::rgb(200, 200, 200))
                                .font_size(10.0);
                        })
                        .class("knob-container");
                    });

                    HStack::new(cx, |cx| {
                        WaveformDisplay::new(cx, false, true)
                            .width(Stretch(1.0))
                            .height(Pixels(40.0))
                            .class("waveform-display");

                        Label::new(cx, "TIME: 0.0s / 10.0s")
                            .color(Color::rgb(136, 136, 136))
                            .font_size(11.0);
                    })
                    .height(Pixels(40.0));
                })
                .class("main-area");
            })
            .class("content-area");

            HStack::new(cx, |cx| {
                Label::new(cx, "RAM > R1 > REC")
                    .color(Color::rgb(136, 136, 136))
                    .font_size(11.0);

                Element::new(cx).width(Stretch(1.0));

                HStack::new(cx, |cx| {
                    for (i, label) in ["R1", "R2", "R3", "R4"].iter().enumerate() {
                        let buffer_index = i as i32;
                        let label_text = *label;
                        HStack::new(cx, |cx| {
                            Label::new(cx, label_text)
                                .color(EditorData::params.map(move |p| {
                                    if p.buffer.value() as i32 == buffer_index {
                                        Color::rgb(0, 0, 0)
                                    } else {
                                        Color::rgb(136, 136, 136)
                                    }
                                }))
                                .font_size(10.0);
                        })
                        .background_color(EditorData::params.map(move |p| {
                            if p.buffer.value() as i32 == buffer_index {
                                Color::rgb(0, 255, 204)
                            } else {
                                Color::rgb(42, 42, 42)
                            }
                        }))
                        .width(Pixels(32.0))
                        .height(Pixels(20.0))
                        .border_radius(Pixels(2.0))
                        .child_space(Stretch(1.0));
                    }
                })
                .col_between(Pixels(4.0));
            })
            .class("footer");
        })
        .class("main-container");
    })
}
