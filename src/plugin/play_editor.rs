use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;
use std::time::Duration;

use crate::machines::shared::{get_global_registry, play_midi_note_active};
use crate::params::RamPlayParams;
use crate::widgets::{waveform_display, RotaryKnob, VerticalSlider, WaveformData};

const COMMIT_HASH: &str = match option_env!("ULTRAWAVE_GIT_HASH") {
    Some(hash) => hash,
    None => "unknown",
};

#[derive(Lens)]
struct EditorData {
    params: Arc<RamPlayParams>,
    waveform: WaveformData,
    status_text: String,
    midi_note_active: bool,
}

enum EditorEvent {
    PollBuffer,
}

impl Model for EditorData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|editor_event, _| match editor_event {
            EditorEvent::PollBuffer => {
                let channel = self.params.buffer.value().clamp(0, 3) as usize;
                let samples = get_global_registry()
                    .read_buffer(channel)
                    .unwrap_or_default();
                let sample_count = samples.len();
                let seconds = sample_count as f32 / 44_100.0;

                let raw_start = self.params.strt.value().clamp(0, 127) as f32 / 127.0;
                let raw_end = self.params.end.value().clamp(0, 127) as f32 / 127.0;
                let (start_pos, end_pos) = if raw_start <= raw_end {
                    (raw_start, raw_end)
                } else {
                    (raw_end, raw_start)
                };

                self.waveform.samples = Arc::new(samples);
                self.waveform.start_pos = start_pos;
                self.waveform.end_pos = end_pos;
                self.status_text = if sample_count == 0 {
                    "EMPTY | 0 smp | 0.00s".to_string()
                } else {
                    format!("READY | {sample_count} smp | {seconds:.2}s")
                };
                self.midi_note_active = play_midi_note_active();
            }
        });
    }
}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (700, 400))
}

pub fn create(
    params: Arc<RamPlayParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        let poll_timer = cx.add_timer(Duration::from_millis(200), None, |cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                cx.emit(EditorEvent::PollBuffer);
            }
        });

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
            .midi-led {
                width: 6px;
                height: 6px;
                border: 1px solid #ffffff;
                background-color: #000000;
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
            .knob-grid {
                border: 2px solid #ffffff;
                background-color: #000000;
            }
            .knob-row {
                border-bottom: 2px solid #ffffff;
            }
            .knob-row:last-child {
                border-bottom: none;
            }
            .knob-container {
                width: 125px;
                height: 110px;
                child-space: 6px;
                background-color: #000000;
                border-right: 2px solid #ffffff;
                text-align: center;
            }
            .knob-container:last-child {
                border-right: none;
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
            .commit-hash {
                color: #777777;
                font-size: 8px;
            }
        "#,
        )
        .expect("Failed to load stylesheet");

        EditorData {
            params: params.clone(),
            waveform: WaveformData::default(),
            status_text: "EMPTY | 0 smp | 0.00s".to_string(),
            midi_note_active: false,
        }
        .build(cx);
        cx.start_timer(poll_timer);
        cx.emit(EditorEvent::PollBuffer);

        VStack::new(cx, |cx| {
            Label::new(cx, "ULTRAWAVE PLAY")
                .font_family(vec![FamilyOwned::Name("DejaVu Sans".to_string())])
                .font_size(18.0)
                .class("title");

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    // 3 vertical sliders side by side (LEV, SRR, DST)
                    HStack::new(cx, |cx| {
                        // LEV slider
                        VStack::new(cx, |cx| {
                            Label::new(cx, "LEV")
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0)
                                .class("knob-label");
                            VerticalSlider::new(cx, EditorData::params, |p| &p.lev)
                                .width(Pixels(24.0))
                                .height(Pixels(100.0));
                            Label::new(cx, EditorData::params.map(|p| format!("{:.1}", p.lev)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(9.0);
                        })
                        .child_space(Stretch(1.0));

                        // SRR slider
                        VStack::new(cx, |cx| {
                            Label::new(cx, "SRR")
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0)
                                .class("knob-label");
                            VerticalSlider::new(cx, EditorData::params, |p| &p.srr)
                                .width(Pixels(24.0))
                                .height(Pixels(100.0));
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.srr)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(9.0);
                        })
                        .child_space(Stretch(1.0));

                        // DIST slider
                        VStack::new(cx, |cx| {
                            Label::new(cx, "DST")
                                .color(Color::rgb(255, 255, 255))
                                .font_size(10.0)
                                .class("knob-label");
                            VerticalSlider::new(cx, EditorData::params, |p| &p.dist)
                                .width(Pixels(24.0))
                                .height(Pixels(100.0));
                            Label::new(cx, EditorData::params.map(|p| format!("{}", p.dist)))
                                .color(Color::rgb(255, 255, 255))
                                .font_size(9.0);
                        })
                        .child_space(Stretch(1.0));
                    })
                    .col_between(Pixels(4.0));

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
                    Label::new(cx, EditorData::status_text)
                        .color(Color::rgb(255, 255, 255))
                        .font_size(9.0);
                })
                .class("sidebar");

                VStack::new(cx, |cx| {
                    // Knob grid - 2 rows x 4 knobs (checkerboard style with shared borders)
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            VStack::new(cx, |cx| {
                                Label::new(cx, "PTCH").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.pitch)
                                    .class("knob")
                                    .width(Pixels(60.0))
                                    .height(Pixels(60.0));
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.pitch)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "DEC").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.dec).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.dec)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "HOLD").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.hold).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.hold)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "BRR").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.srr).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.srr)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));
                        })
                        .col_between(Pixels(0.0))
                        .class("knob-row");

                        HStack::new(cx, |cx| {
                            VStack::new(cx, |cx| {
                                Label::new(cx, "STRT").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.strt).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.strt)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "END").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.end)
                                    .class("knob")
                                    .width(Pixels(60.0))
                                    .height(Pixels(60.0));
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.end)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "RTRG").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.rtrg).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.rtrg)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                Label::new(cx, "RTIM").class("knob-label");
                                RotaryKnob::new(cx, EditorData::params, |p| &p.rtim).class("knob");
                                Label::new(cx, EditorData::params.map(|p| format!("{}", p.rtim)))
                                    .color(Color::rgb(255, 255, 255))
                                    .font_size(10.0);
                            })
                            .class("knob-container")
                            .child_space(Stretch(1.0));
                        })
                        .col_between(Pixels(0.0))
                        .class("knob-row");
                    })
                    .class("knob-grid")
                    .row_between(Pixels(0.0));

                    HStack::new(cx, |cx| {
                        waveform_display(cx, EditorData::waveform, true, false)
                            .width(Stretch(1.0))
                            .height(Pixels(40.0))
                            .class("waveform-display");

                        Label::new(cx, "[▌▐] STRT/END")
                            .color(Color::rgb(255, 255, 255))
                            .font_size(11.0);
                    })
                    .height(Pixels(40.0))
                    .col_between(Pixels(8.0));
                })
                .row_between(Pixels(0.0))
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
                                    if p.buffer.value() == buffer_index {
                                        Color::rgb(0, 0, 0)
                                    } else {
                                        Color::rgb(255, 255, 255)
                                    }
                                }))
                                .font_size(10.0);
                        })
                        .background_color(EditorData::params.map(move |p| {
                            if p.buffer.value() == buffer_index {
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

                Element::new(cx)
                    .class("midi-led")
                    .background_color(EditorData::midi_note_active.map(|active| {
                        if *active {
                            Color::rgb(255, 255, 255)
                        } else {
                            Color::rgb(0, 0, 0)
                        }
                    }));

                Element::new(cx).width(Stretch(1.0));

                Label::new(cx, COMMIT_HASH).class("commit-hash");
            })
            .col_between(Pixels(8.0))
            .class("footer");
        })
        .class("main-container");
    })
}
