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
                width: 120px;
                background-color: #0d0d0d;
                child-space: 8px;
            }
            .knob-label {
                color: #00ffcc;
                font-size: 11px;
            }
            .knob-container {
                width: 90px;
                height: 110px;
                child-space: 6px;
                background-color: #0a0a0a;
            }
            .main-area {
                background-color: #2a2a2a;
                child-space: 15px;
            }
            .waveform-display {
                width: 400px;
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
            Label::new(cx, "ULTRAWAVE PLAY")
                .font_family(vec![FamilyOwned::Name("DejaVu Sans".to_string())])
                .font_size(18.0)
                .class("title");

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.lev)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0));
                        Label::new(cx, "LEV")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0);
                    });

                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.srr)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0));
                        Label::new(cx, "SRR")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0);
                    });

                    VStack::new(cx, |cx| {
                        ParamSlider::new(cx, EditorData::params, |p| &p.dist)
                            .width(Pixels(50.0))
                            .height(Pixels(50.0));
                        Label::new(cx, "DIST")
                            .color(Color::rgb(0, 255, 204))
                            .font_size(10.0);
                    });

                    Label::new(cx, "LEVEL")
                        .color(Color::rgb(136, 136, 136))
                        .font_size(10.0);
                    Element::new(cx)
                        .width(Pixels(20.0))
                        .height(Pixels(80.0))
                        .background_color(Color::rgb(10, 10, 10));

                    Label::new(cx, "P01")
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
                    .color(Color::rgb(0, 255, 204))
                    .font_size(9.0);
                })
                .class("sidebar");

                VStack::new(cx, |cx| {
                    // Knob grid - 2 rows x 4 knobs
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "PTCH").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.pitch)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "DEC").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.dec)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "HOLD").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.hold)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "BRR").class("knob-label");
                            Element::new(cx)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0))
                                .background_color(Color::rgb(26, 26, 26));
                        })
                        .class("knob-container");
                    });

                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "STRT").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.strt)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "END").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.end)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTRG").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rtrg)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");

                        VStack::new(cx, |cx| {
                            Label::new(cx, "RTIM").class("knob-label");
                            ParamSlider::new(cx, EditorData::params, |p| &p.rtim)
                                .width(Pixels(60.0))
                                .height(Pixels(60.0));
                        })
                        .class("knob-container");
                    });

                    WaveformDisplay::new(cx, true, false)
                        .width(Pixels(400.0))
                        .height(Pixels(40.0))
                        .class("waveform-display");
                })
                .class("main-area");
            })
            .class("content-area");

            HStack::new(cx, |cx| {
                Label::new(cx, "RAM > R1 > PLAY")
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
