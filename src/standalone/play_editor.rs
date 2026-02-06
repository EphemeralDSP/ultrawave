use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::RamPlayParams;
use crate::widgets::{WaveformData, WaveformDisplay};

#[derive(Lens)]
struct EditorData {
    params: Arc<RamPlayParams>,
    waveform: WaveformData,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (700, 400))
}

#[derive(Debug, Clone, Copy)]
struct BufferSelectEvent(i32);

pub fn create(
    params: Arc<RamPlayParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSans.ttf"));
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"));
        cx.set_default_font(&["DejaVu Sans", "SansSerif", "DejaVu Sans Mono"]);

        // 1-bit monochrome hardware aesthetic
        cx.add_stylesheet(
            r#"
            .main-container {
                background-color: #000000;
                width: 100%;
                height: 100%;
                child-space: 10px;
            }
            .title {
                color: #ffffff;
                font-size: 18px;
                height: 30px;
            }
            .sidebar {
                width: 140px;
                background-color: #000000;
                child-space: 16px;
                border-right: 2px solid #ffffff;
            }
            .knob-label {
                color: #ffffff;
                font-size: 11px;
                text-align: center;
                margin-bottom: 4px;
            }
            .knob-container {
                width: 125px;
                height: 110px;
                child-space: 6px;
                background-color: #000000;
                border: 2px solid #ffffff;
            }
            .knob {
                width: 60px;
                height: 60px;
                background-color: #000000;
                border: 2px solid #ffffff;
            }
            .knob:hover {
                border-color: #ffffff;
                background-color: #333333;
            }
            .knob slider {
                height: 60px;
                width: 60px;
            }
            .knob .track {
                background-color: transparent;
            }
            .knob .fill {
                background-color: transparent;
            }
            .knob .thumb {
                background-color: #ffffff;
                width: 8px;
                height: 8px;
                border: 1px solid #ffffff;
            }
            .knob label {
                background-color: transparent;
                color: #ffffff;
            }
            .knob .thumb:hover {
                background-color: #ffffff;
            }
            .main-area {
                background-color: #000000;
                width: Stretch(1.0);
                child-space: 15px;
            }
            .waveform-display {
                width: 100%;
                height: 40px;
                background-color: #000000;
                border: 1px solid #ffffff;
            }
            .footer {
                height: 35px;
                background-color: #000000;
                border-top: 2px solid #ffffff;
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
            Label::new(cx, "ULTRAWAVE PLAY")
                .font_family(vec![FamilyOwned::Name("DejaVu Sans".to_string())])
                .font_size(18.0)
                .class("title");

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, "LEV")
                            .color(Color::rgb(255, 255, 255))
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
                            .color(Color::rgb(255, 255, 255))
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
                            .color(Color::rgb(255, 255, 255))
                            .font_size(10.0)
                            .class("knob-label");
                        ParamSlider::new(cx, EditorData::params, |p| &p.dist)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0))
                            .class("knob");
                    })
                    .child_space(Stretch(0.5));

                    Label::new(cx, "LEVEL")
                        .color(Color::rgb(255, 255, 255))
                        .font_size(10.0);
                    Element::new(cx)
                        .width(Pixels(20.0))
                        .height(Pixels(80.0))
                        .background_color(Color::rgb(0, 0, 0))
                        .border_color(Color::rgb(255, 255, 255))
                        .border_width(Pixels(1.0));

                    // Buffer status with bar indicators (▌▌ P01 ▐▐)
                    Label::new(cx, "\u{258c}\u{258c} P01 \u{2590}\u{2590}")
                        .color(Color::rgb(255, 255, 255))
                        .font_size(14.0);
                    Label::new(
                        cx,
                        EditorData::params.map(|p| {
                            let buffer_name = match p.buffer.value() {
                                0 => "R1",
                                1 => "R2",
                                2 => "R3",
                                3 => "R4",
                                _ => "R1",
                            };
                            format!("SRC: {}", buffer_name)
                        }),
                    )
                    .color(Color::rgb(255, 255, 255))
                    .font_size(9.0);
                })
                .class("sidebar");

                VStack::new(cx, |cx| {
                    // Knob grid - 2 rows x 4 knobs
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "PTCH").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.pitch)
                                .class("knob")
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.pitch)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "DEC").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.dec).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.dec)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "HOLD").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.hold).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.hold)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "BRR").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.srr).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.srr)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");
                    });

                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "STRT").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.strt).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.strt)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "END").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.end)
                                .class("knob")
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.end)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTRG").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rtrg).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.rtrg)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTIM").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rtim).class("knob");
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.rtim)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0);
                        })
                        .class("knob-container");
                    });

                    HStack::new(cx, |cx| {
                        WaveformDisplay::new(cx, true, false)
                            .width(Stretch(1.0))
                            .height(Pixels(40.0))
                            .class("waveform-display");

                        Label::new(cx, "TIME: 0.0s / 10.0s")
                            .color(Color::rgb(255, 255, 255))
                            .font_size(11.0);
                    })
                    .height(Pixels(40.0));
                })
                .class("main-area");
            })
            .class("content-area");

            HStack::new(cx, |cx| {
                Label::new(cx, "RAM > R1 > PLAY")
                    .color(Color::rgb(255, 255, 255))
                    .font_size(11.0);

                Element::new(cx).width(Stretch(1.0));

                HStack::new(cx, |cx| {
                    for (i, label) in ["[R1]", "[R2]", "[R3]", "[R4]"].iter().enumerate() {
                        let buffer_index = i as i32;
                        let label_text = *label;
                        HStack::new(cx, |cx| {
                            Label::new(cx, label_text)
                                .color(EditorData::params.map(move |p| {
                                    if p.buffer.value() as i32 == buffer_index {
                                        Color::rgb(0, 0, 0)
                                    } else {
                                        Color::rgb(255, 255, 255)
                                    }
                                }))
                                .font_size(10.0);
                        })
                        .background_color(EditorData::params.map(move |p| {
                            if p.buffer.value() as i32 == buffer_index {
                                Color::rgb(255, 255, 255)
                            } else {
                                Color::rgb(0, 0, 0)
                            }
                        }))
                        .border_color(Color::rgb(255, 255, 255))
                        .border_width(Pixels(1.0))
                        .width(Pixels(40.0))
                        .height(Pixels(20.0))
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
