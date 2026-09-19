use egui::{
    self, vec2, Color32, FontData, FontDefinitions, FontFamily, FontId, Layout, RichText, Stroke,
    StrokeKind, UiBuilder,
};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::LogicalSize;
use nice_plug::prelude::{FloatParam, IntParam, Param};
use nice_plug_egui::{
    create_egui_editor, EguiEditor, EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp,
    RepaintNotifier,
};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::time::Duration;

use super::UiState;
use crate::machines::shared::BufferRegistry;
use crate::params::{RamPlayParams, RamRecordParams};
use crate::widgets::{rotary_knob, vertical_slider, waveform};

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
    fn bank(&self) -> &IntParam {
        match self {
            Self::Record(p) => &p.bank,
            Self::Play(p) => &p.bank,
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

pub struct UltrawaveEditorApp {
    params: EditorParams,
    ui_state: Arc<UiState>,
    gui: Option<GuiContext>,
    waveform: Arc<Vec<i16>>,
    metadata: Option<(usize, usize, u64)>,
    meter: f32,
    registry: Arc<Mutex<Arc<BufferRegistry>>>,
}

pub type PluginEditor = EguiEditor<UltrawaveEditorApp>;

pub fn default_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(LogicalSize::new(760.0, 460.0), 1.0)
}

pub fn create(
    params: EditorParams,
    state: Arc<EguiEditorState>,
    ui: Arc<UiState>,
    registry: Arc<Mutex<Arc<BufferRegistry>>>,
) -> Option<PluginEditor> {
    let repaint = RepaintNotifier::new();
    create_egui_editor(
        state,
        repaint.clone(),
        EguiNiceSettings::new().with_tile("Ultrawave"),
        UltrawaveEditorApp {
            params,
            ui_state: ui,
            gui: None,
            waveform: Arc::new(Vec::new()),
            metadata: None,
            meter: 0.0,
            registry,
        },
    )
}

impl NiceEguiApp for UltrawaveEditorApp {
    fn build(
        &mut self,
        ctx: egui::Context,
        gui: GuiContext,
        _frame: &mut Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        self.gui = Some(gui);
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "uw-sans".into(),
            Arc::new(FontData::from_static(include_bytes!(
                "../../assets/fonts/DejaVuSans.ttf"
            ))),
        );
        fonts.font_data.insert(
            "uw-mono".into(),
            Arc::new(FontData::from_static(include_bytes!(
                "../../assets/fonts/DejaVuSansMono.ttf"
            ))),
        );
        fonts
            .families
            .insert(FontFamily::Monospace, vec!["uw-mono".into()]);
        fonts
            .families
            .insert(FontFamily::Proportional, vec!["uw-sans".into()]);
        ctx.set_fonts(fonts);
        ctx.set_theme(egui::Theme::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::BLACK;
        visuals.window_fill = Color32::BLACK;
        visuals.widgets.noninteractive.bg_fill = Color32::BLACK;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.inactive.bg_fill = Color32::BLACK;
        visuals.widgets.inactive.weak_bg_fill = Color32::BLACK;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.hovered.bg_fill = Color32::from_gray(45);
        visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(45);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.bg_fill = Color32::WHITE;
        visuals.widgets.active.weak_bg_fill = Color32::WHITE;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::BLACK);
        visuals.selection.bg_fill = Color32::WHITE;
        visuals.selection.stroke = Stroke::new(1.0, Color32::BLACK);
        visuals.widgets.inactive.corner_radius = 0.into();
        visuals.widgets.hovered.corner_radius = 0.into();
        visuals.widgets.active.corner_radius = 0.into();
        ctx.set_visuals(visuals);
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.override_font_id = Some(FontId::monospace(12.0));
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
        let Some(gui) = self.gui.clone() else { return };
        let channel = self.params.buffer().unmodulated_plain_value() as usize;
        let registry = self
            .registry
            .lock()
            .expect("bank UI registry poisoned")
            .clone();
        let (len, rate, revision) = registry.snapshot(channel).unwrap_or((0, 44_100.0, 0));
        let metadata = Some((channel, len, revision));
        if metadata != self.metadata {
            if let Some(samples) = registry.read_buffer(channel) {
                self.waveform = Arc::new(samples);
                self.metadata = metadata;
            }
        }
        let peak = f32::from_bits(self.ui_state.peak.swap(0, Ordering::Relaxed));
        let db = if peak > 0.000_001 {
            20.0 * peak.log10()
        } else {
            -120.0
        };
        self.meter = ((db + 60.0) / 60.0).clamp(0.0, 1.0).max(self.meter * 0.75);
        let is_record = matches!(self.params, EditorParams::Record(_));

        // Fixed logical sections preserve the hardware panel at every host DPI. Explicit
        // bounds keep an expanding horizontal layout from pushing the waveform/footer out.
        let origin = ui.max_rect().min;
        let rect = |x, y, w, h| egui::Rect::from_min_size(origin + vec2(x, y), vec2(w, h));
        ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::BLACK);
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(14.0, 14.0, 732.0, 34.0)),
            |ui| {
                self.header(ui, &gui, is_record);
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(14.0, 60.0, 124.0, 244.0)),
            |ui| {
                self.sidebar(ui, &gui, is_record, db);
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(154.0, 60.0, 592.0, 244.0)),
            |ui| {
                self.knob_grid(ui, &gui, is_record);
            },
        );
        let markers = match &self.params {
            EditorParams::Play(p) => Some((
                p.strt.unmodulated_plain_value() as f32 / 127.0,
                p.end.unmodulated_plain_value() as f32 / 127.0,
            )),
            _ => None,
        };
        waveform(ui, rect(14.0, 316.0, 732.0, 66.0), &self.waveform, markers);
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(14.0, 392.0, 732.0, 18.0)),
            |ui| {
                self.status(ui, len, rate, is_record);
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(14.0, 418.0, 732.0, 28.0)),
            |ui| {
                self.footer(ui, &gui, is_record);
            },
        );
    }

    fn editor_closed(&mut self) {
        self.gui = None;
    }
}

impl UltrawaveEditorApp {
    fn header(&self, ui: &mut egui::Ui, gui: &GuiContext, record: bool) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("ULTRAWAVE").font(FontId::monospace(21.0)));
            ui.label(if record {
                "RAM RECORD / 12-BIT"
            } else {
                "RAM PLAY / 12-BIT"
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [120.0, 28.0],
                        egui::Button::new(if record {
                            "● CAPTURE"
                        } else {
                            "▶ AUDITION"
                        }),
                    )
                    .clicked()
                {
                    self.ui_state.trigger.store(true, Ordering::Relaxed);
                }
                if let EditorParams::Record(p) = &self.params {
                    let locked = self.ui_state.capture_locked.load(Ordering::Relaxed);
                    let active = self.ui_state.active.load(Ordering::Relaxed);
                    if ui
                        .add_enabled(
                            locked && !active && p.one_shot.unmodulated_plain_value(),
                            egui::Button::new("RE-ARM").min_size(vec2(80.0, 28.0)),
                        )
                        .clicked()
                    {
                        self.ui_state.rearm.store(true, Ordering::Relaxed);
                    }
                    let selected = p.one_shot.unmodulated_plain_value();
                    if ui
                        .add_sized([94.0, 28.0], egui::Button::selectable(selected, "ONE SHOT"))
                        .clicked()
                    {
                        let setter = gui.param_setter();
                        setter.begin_set_parameter(&p.one_shot);
                        setter.set_parameter(&p.one_shot, !selected);
                        setter.end_set_parameter(&p.one_shot);
                    }
                }
            });
        });
    }

    fn sidebar(&self, ui: &mut egui::Ui, gui: &GuiContext, record: bool, db: f32) {
        ui.vertical(|ui| {
            ui.set_width(124.0);
            ui.horizontal_top(|ui| {
                for (label, kind) in [("LEV", 0), ("SRR", 1), ("DIST", 2)] {
                    ui.allocate_ui_with_layout(
                        vec2(36.0, 164.0),
                        Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.label(label);
                            match kind {
                                0 => {
                                    vertical_slider(ui, self.params.level(), gui);
                                }
                                1 => {
                                    vertical_slider(ui, self.params.srr(), gui);
                                }
                                _ => {
                                    vertical_slider(ui, self.params.distortion(), gui);
                                }
                            }
                            let value = match kind {
                                0 => {
                                    format!("{:.0}", self.params.level().unmodulated_plain_value())
                                }
                                1 => format!("{:.2}", self.params.srr().unmodulated_plain_value()),
                                _ => self.params.distortion().normalized_value_to_string(
                                    self.params.distortion().unmodulated_normalized_value(),
                                    false,
                                ),
                            };
                            ui.label(value);
                        },
                    );
                }
            });
            ui.label(if record {
                "CAPTURE LEVEL"
            } else {
                "OUTPUT LEVEL"
            });
            let (rect, _) = ui.allocate_exact_size(vec2(124.0, 7.0), egui::Sense::hover());
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
            let mut fill = rect.shrink(1.0);
            fill.set_right(fill.left() + fill.width() * self.meter);
            ui.painter().rect_filled(fill, 0.0, Color32::WHITE);
            ui.label(if db <= -60.0 {
                "−∞ dBFS".into()
            } else {
                format!("{db:.1} dBFS")
            });
        });
    }

    fn knob_grid(&self, ui: &mut egui::Ui, gui: &GuiContext, record: bool) {
        let labels = if record {
            [
                "MLEV", "MBAL", "ILEV", "IBAL", "CUE1", "CUE2", "LEN", "RATE",
            ]
        } else {
            ["PTCH", "DEC", "HOLD", "BRR", "STRT", "END", "RTRG", "RTIM"]
        };
        let origin = ui.max_rect().min;
        for (index, label) in labels.iter().enumerate() {
            let cell = egui::Rect::from_min_size(
                origin + vec2((index % 4) as f32 * 148.0, (index / 4) as f32 * 122.0),
                vec2(148.0, 122.0),
            );
            ui.painter().rect_stroke(
                cell,
                0.0,
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(cell.shrink(8.0))
                    .layout(Layout::top_down(egui::Align::Center)),
                |ui| {
                    ui.label(*label);
                    rotary_knob(ui, self.params.knob(index), gui);
                    ui.label(self.params.knob(index).normalized_value_to_string(
                        self.params.knob(index).unmodulated_normalized_value(),
                        false,
                    ));
                },
            );
        }
    }

    fn status(&self, ui: &mut egui::Ui, len: usize, rate: f32, record: bool) {
        let active = self.ui_state.active.load(Ordering::Relaxed);
        let one_shot =
            matches!(&self.params, EditorParams::Record(p) if p.one_shot.unmodulated_plain_value());
        let locked = self.ui_state.capture_locked.load(Ordering::Relaxed);
        let phase = if active {
            if record {
                "RECORDING"
            } else {
                "PLAYING"
            }
        } else if one_shot && locked {
            "LOCKED — RE-ARM TO CAPTURE"
        } else if one_shot {
            "ARMED — NEXT TRIGGER RECORDS"
        } else if len == 0 {
            "EMPTY — CAPTURE TO BEGIN"
        } else {
            "READY"
        };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(7.0, 7.0), egui::Sense::hover());
            ui.painter().rect(
                r,
                0.0,
                if active {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
            ui.label(format!(
                "{phase}   ·   {:.2} s   ·   {:.1} kHz",
                len as f32 / rate.max(1.0),
                rate / 1000.0
            ));
        });
    }

    fn footer(&self, ui: &mut egui::Ui, gui: &GuiContext, record: bool) {
        ui.horizontal(|ui| {
            let selected = self.params.buffer().unmodulated_plain_value();
            let bank = self.params.bank().unmodulated_plain_value();
            let bank_status = self.ui_state.bank_status.load(Ordering::Relaxed);
            let bank_text = match bank_status {
                1 => format!("B{bank}…"),
                3 => format!("B{bank}!"),
                _ if bank == 0 => "OFF".into(),
                _ => format!("B{bank}"),
            };
            ui.label(format!(
                "▌▌ R{} ▐▐  {}",
                selected + 1,
                if record { "REC" } else { "PLAY" }
            ));
            ui.label("BANK");
            if ui
                .add_sized([48.0, 28.0], egui::Button::new(bank_text))
                .clicked()
            {
                let next = if bank >= 16 { 0 } else { bank + 1 };
                let p = self.params.bank();
                let setter = gui.param_setter();
                setter.begin_set_parameter(p);
                setter.set_parameter(p, next);
                setter.end_set_parameter(p);
            }
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                for index in (0..4).rev() {
                    if ui
                        .add_sized(
                            [44.0, 28.0],
                            egui::Button::selectable(
                                selected == index,
                                format!("[R{}]", index + 1),
                            ),
                        )
                        .clicked()
                    {
                        let p = self.params.buffer();
                        let setter = gui.param_setter();
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, index);
                        setter.end_set_parameter(p);
                    }
                }
            });
        });
    }
}
