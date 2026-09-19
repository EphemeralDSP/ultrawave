use nih_plug::prelude::{Editor, FloatParam, IntParam};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::ParamEvent;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use super::UiState;
use crate::machines::shared::get_global_registry;
use crate::params::{RamPlayParams, RamRecordParams};
use crate::widgets::{waveform_display, RotaryKnob, VerticalSlider, WaveformData};

#[derive(Clone)]
pub enum EditorParams {
    Record(Arc<RamRecordParams>),
    Play(Arc<RamPlayParams>),
}

impl EditorParams {
    fn buffer(&self) -> &IntParam {
        match self {
            Self::Record(p) => &p.buffer,
            Self::Play(p) => &p.buffer,
        }
    }
    fn level(&self) -> &FloatParam {
        match self {
            Self::Record(p) => &p.lev,
            Self::Play(p) => &p.lev,
        }
    }
    fn srr(&self) -> &FloatParam {
        match self {
            Self::Record(p) => &p.srr,
            Self::Play(p) => &p.srr,
        }
    }
    fn distortion(&self) -> &IntParam {
        match self {
            Self::Record(p) => &p.dist,
            Self::Play(p) => &p.dist,
        }
    }
    fn knob(&self, index: usize) -> &IntParam {
        match self {
            Self::Record(p) => [
                &p.mlev,
                &p.mbal,
                &p.ilev,
                &p.ibal,
                &p.cue1,
                &p.cue2,
                &p.rec_len,
                &p.rec_rate,
            ][index],
            Self::Play(p) => [
                &p.pitch, &p.dec, &p.hold, &p.brr, &p.strt, &p.end, &p.rtrg, &p.rtim,
            ][index],
        }
    }
}

#[derive(Lens)]
struct EditorData {
    params: EditorParams,
    ui: Arc<UiState>,
    waveform: WaveformData,
    metadata: Option<(usize, usize, u64)>,
    status: String,
    meter: f32,
    meter_text: String,
    active: bool,
}

enum EditorEvent {
    Poll,
    SelectBuffer(i32),
    Trigger,
}

impl Model for EditorData {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|event, _| match event {
            EditorEvent::Trigger => self.ui.trigger.store(true, Ordering::Relaxed),
            EditorEvent::SelectBuffer(index) => {
                let param = self.params.buffer();
                cx.emit(ParamEvent::BeginSetParameter(param).upcast());
                cx.emit(ParamEvent::SetParameter(param, *index).upcast());
                cx.emit(ParamEvent::EndSetParameter(param).upcast());
            }
            EditorEvent::Poll => {
                let channel = self.params.buffer().value() as usize;
                let registry = get_global_registry();
                let (len, rate, revision) = registry.snapshot(channel).unwrap_or((0, 44100.0, 0));
                let metadata = Some((channel, len, revision));
                if self.metadata != metadata {
                    if let Some(samples) = registry.read_buffer(channel) {
                        self.waveform.samples = Arc::new(samples);
                        self.metadata = metadata;
                    }
                }
                if let EditorParams::Play(p) = &self.params {
                    self.waveform.start_pos = p.strt.value() as f32 / 127.0;
                    self.waveform.end_pos = p.end.value() as f32 / 127.0;
                }
                self.active = self.ui.active.load(Ordering::Relaxed);
                let phase = if self.active {
                    match self.params {
                        EditorParams::Record(_) => "RECORDING",
                        EditorParams::Play(_) => "PLAYING",
                    }
                } else if len == 0 {
                    "EMPTY — CAPTURE TO BEGIN"
                } else {
                    "READY"
                };
                self.status = format!(
                    "{phase}   ·   {:.2} s   ·   {:.1} kHz",
                    len as f32 / rate.max(1.0),
                    rate / 1000.0
                );
                let peak = f32::from_bits(self.ui.peak.swap(0, Ordering::Relaxed));
                let db = if peak > 0.000_001 {
                    20.0 * peak.log10()
                } else {
                    -120.0
                };
                self.meter = ((db + 60.0) / 60.0).clamp(0.0, 1.0).max(self.meter * 0.75);
                self.meter_text = if db <= -60.0 {
                    "−∞ dBFS".into()
                } else {
                    format!("{db:.1} dBFS")
                };
            }
        });
    }
}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (760, 460))
}

pub fn create(
    params: EditorParams,
    state: Arc<ViziaState>,
    ui: Arc<UiState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(state, ViziaTheming::Custom, move |cx, _| {
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSans.ttf"));
        cx.add_font_mem(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"));
        cx.set_default_font(&["DejaVu Sans Mono"]);
        cx.add_stylesheet(r#"
            .root { background-color: #000; color: #fff; child-space: 14px; row-between: 12px; }
            label { color: #fff; font-size: 12; }
            .header { height: 34px; col-between: 12px; child-top: 1s; child-bottom: 1s; }
            .title { font-size: 21; }
            .role { font-size: 12; }
            button { background-color: #000; color: #fff; border: 1px solid #fff; border-radius: 0px; child-space: 1s; height: 28px; }
            button:hover { background-color: #333; }
            button:checked { background-color: #fff; }
            button:checked label { color: #000; }
            .content { height: 244px; col-between: 16px; }
            .sidebar { width: 124px; row-between: 10px; }
            .faders { height: 164px; col-between: 8px; }
            .fader { child-left: 1s; child-right: 1s; row-between: 8px; }
            .readout { font-size: 12; }
            .grid { border: 1px solid #fff; }
            .knob-row { height: 1s; }
            .cell { width: 1s; child-space: 1s; row-between: 4px; border: 1px solid #fff; }
            .knob { width: 58px; height: 58px; }
            .meter-track { height: 7px; border: 1px solid #fff; }
            .meter-fill { background-color: #fff; height: 100%; }
            .wave { height: 66px; }
            .status { height: 18px; }
            .footer { height: 28px; col-between: 6px; child-top: 1s; child-bottom: 1s; }
            .led { width: 7px; height: 7px; border: 1px solid #fff; }
        "#).expect("editor stylesheet");

        let is_record = matches!(params, EditorParams::Record(_));
        EditorData {
            params: params.clone(),
            ui: ui.clone(),
            waveform: WaveformData::default(),
            metadata: None,
            status: "EMPTY — CAPTURE TO BEGIN".into(),
            meter: 0.0,
            meter_text: "−∞ dBFS".into(),
            active: false,
        }
        .build(cx);
        cx.emit(EditorEvent::Poll);
        // This baseview backend does not service Vizia timers. Stop when the editor proxy closes.
        let mut proxy = cx.get_proxy();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(100));
            if proxy.emit(EditorEvent::Poll).is_err() {
                break;
            }
        });

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "ULTRAWAVE").class("title");
                Label::new(
                    cx,
                    if is_record {
                        "RAM RECORD / 12-BIT"
                    } else {
                        "RAM PLAY / 12-BIT"
                    },
                )
                .class("role");
                Element::new(cx).width(Stretch(1.0));
                Button::new(
                    cx,
                    move |cx| cx.emit(EditorEvent::Trigger),
                    |cx| {
                        Label::new(
                            cx,
                            if is_record {
                                "● CAPTURE"
                            } else {
                                "▶ AUDITION"
                            },
                        )
                    },
                )
                .width(Pixels(120.0));
            })
            .class("header");

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            Label::new(cx, "LEV");
                            VerticalSlider::new(cx, EditorData::params, EditorParams::level)
                                .width(Pixels(24.0))
                                .height(Pixels(116.0));
                            Label::new(
                                cx,
                                EditorData::params.map(|p| format!("{:.0}", p.level().value())),
                            )
                            .class("readout");
                        })
                        .class("fader");
                        VStack::new(cx, |cx| {
                            Label::new(cx, "SRR");
                            VerticalSlider::new(cx, EditorData::params, EditorParams::srr)
                                .width(Pixels(24.0))
                                .height(Pixels(116.0));
                            Label::new(
                                cx,
                                EditorData::params.map(|p| format!("{:.2}", p.srr().value())),
                            )
                            .class("readout");
                        })
                        .class("fader");
                        VStack::new(cx, |cx| {
                            Label::new(cx, "DIST");
                            VerticalSlider::new(cx, EditorData::params, EditorParams::distortion)
                                .width(Pixels(24.0))
                                .height(Pixels(116.0));
                            Label::new(
                                cx,
                                EditorData::params.map(|p| format!("{}", p.distortion())),
                            )
                            .class("readout");
                        })
                        .class("fader");
                    })
                    .class("faders");
                    Label::new(
                        cx,
                        if is_record {
                            "CAPTURE LEVEL"
                        } else {
                            "OUTPUT LEVEL"
                        },
                    );
                    HStack::new(cx, |cx| {
                        Element::new(cx)
                            .class("meter-fill")
                            .width(EditorData::meter.map(|v| Percentage(v * 100.0)));
                    })
                    .class("meter-track");
                    Label::new(cx, EditorData::meter_text).class("readout");
                })
                .class("sidebar");

                VStack::new(cx, |cx| {
                    let labels = if is_record {
                        [
                            "MLEV", "MBAL", "ILEV", "IBAL", "CUE1", "CUE2", "LEN", "RATE",
                        ]
                    } else {
                        ["PTCH", "DEC", "HOLD", "BRR", "STRT", "END", "RTRG", "RTIM"]
                    };
                    for row in 0..2 {
                        HStack::new(cx, |cx| {
                            for column in 0..4 {
                                let index = row * 4 + column;
                                VStack::new(cx, |cx| {
                                    Label::new(cx, labels[index]);
                                    RotaryKnob::new(cx, EditorData::params, move |p| p.knob(index))
                                        .class("knob");
                                    Label::new(
                                        cx,
                                        EditorData::params
                                            .map(move |p| format!("{}", p.knob(index))),
                                    )
                                    .class("readout");
                                })
                                .class("cell");
                            }
                        })
                        .class("knob-row");
                    }
                })
                .class("grid");
            })
            .class("content");

            waveform_display(cx, EditorData::waveform, !is_record, false).class("wave");
            HStack::new(cx, |cx| {
                Element::new(cx)
                    .class("led")
                    .background_color(EditorData::active.map(|active| {
                        if *active {
                            Color::white()
                        } else {
                            Color::black()
                        }
                    }));
                Label::new(cx, EditorData::status);
            })
            .class("status")
            .col_between(Pixels(10.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
            HStack::new(cx, |cx| {
                Label::new(
                    cx,
                    EditorData::params.map(move |p| {
                        format!(
                            "▌▌ R{} ▐▐  RAM > {}",
                            p.buffer().value() + 1,
                            if is_record {
                                "REC / CUE1 DRY / CUE2 CAPTURE"
                            } else {
                                "PLAY / MONO > STEREO"
                            }
                        )
                    }),
                );
                Element::new(cx).width(Stretch(1.0));
                for index in 0..4 {
                    Button::new(
                        cx,
                        move |cx| cx.emit(EditorEvent::SelectBuffer(index)),
                        move |cx| Label::new(cx, &format!("[R{}]", index + 1)),
                    )
                    .width(Pixels(44.0))
                    .checked(EditorData::params.map(move |p| p.buffer().value() == index));
                }
            })
            .class("footer");
        })
        .class("root");
    })
}
