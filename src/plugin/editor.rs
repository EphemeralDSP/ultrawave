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
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use super::UiState;
use crate::machines::memory::SampleMemory;
use crate::params::UltrawaveParams;
use crate::widgets::{parameter_value, rotary_knob, vertical_slider, waveform, WaveformCache};

pub struct UltrawaveEditorApp {
    params: Arc<UltrawaveParams>,
    record_page: bool,
    ui_state: Arc<UiState>,
    gui: Option<GuiContext>,
    waveform: Arc<Vec<i16>>,
    metadata: Option<(usize, usize, u64)>,
    waveform_cache: WaveformCache,
    meter: f32,
    memory: Arc<SampleMemory>,
}

pub type PluginEditor = EguiEditor<UltrawaveEditorApp>;

pub fn default_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(LogicalSize::new(760.0, 460.0), 1.0)
}

pub fn create(
    params: Arc<UltrawaveParams>,
    state: Arc<EguiEditorState>,
    ui: Arc<UiState>,
    memory: Arc<SampleMemory>,
) -> Option<PluginEditor> {
    let repaint = RepaintNotifier::new();
    create_egui_editor(
        state,
        repaint.clone(),
        EguiNiceSettings::new().with_tile("Ultrawave"),
        UltrawaveEditorApp {
            params,
            record_page: std::env::var("ULTRAWAVE_SCREENSHOT_PAGE").as_deref() != Ok("play"),
            ui_state: ui,
            gui: None,
            waveform: Arc::new(Vec::new()),
            metadata: None,
            waveform_cache: WaveformCache::default(),
            meter: 0.0,
            memory,
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
        let channel = self.params.buffer.unmodulated_plain_value() as usize;
        let snapshot = self.memory.snapshot(channel);
        let (len, rate, revision) = snapshot.unwrap_or((0, 44_100.0, 0));
        let metadata = Some((channel, len, revision));
        if snapshot.is_none() {
            self.waveform = Arc::new(Vec::new());
            self.metadata = None;
        } else if metadata != self.metadata {
            if let Some(samples) = self.memory.read_buffer(channel) {
                self.waveform = Arc::new(samples);
                self.metadata = metadata;
            } else {
                self.waveform = Arc::new(Vec::new());
                self.metadata = None;
            }
        }
        let capture_peak = self.ui_state.capture_peak.swap(0, Ordering::Relaxed);
        let playback_peak = self.ui_state.playback_peak.swap(0, Ordering::Relaxed);
        let peak = f32::from_bits(if self.record_page {
            capture_peak
        } else {
            playback_peak
        });
        let db = if peak > 0.000_001 {
            20.0 * peak.log10()
        } else {
            -120.0
        };
        self.meter = ((db + 60.0) / 60.0).clamp(0.0, 1.0).max(self.meter * 0.75);
        let is_record = self.record_page;

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
        let markers = (!is_record).then_some((&self.params.play.strt, &self.params.play.end));
        let cursor = self.ui_state.playback_position(channel, len);
        waveform(
            ui,
            rect(14.0, 316.0, 732.0, 66.0),
            &self.waveform,
            (Arc::as_ptr(&self.memory) as usize, channel, len, revision),
            &mut self.waveform_cache,
            markers,
            cursor,
            &gui,
        );
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
    fn level(&self) -> &FloatParam {
        if self.record_page {
            &self.params.record.lev
        } else {
            &self.params.play.lev
        }
    }
    fn srr(&self) -> &FloatParam {
        if self.record_page {
            &self.params.record.srr
        } else {
            &self.params.play.srr
        }
    }
    fn distortion(&self) -> &IntParam {
        if self.record_page {
            &self.params.record.dist
        } else {
            &self.params.play.dist
        }
    }
    fn knob(&self, index: usize) -> &IntParam {
        if self.record_page {
            let p = &self.params.record;
            [
                &p.mlev,
                &p.mbal,
                &p.ilev,
                &p.ibal,
                &self.params.record_note,
                &self.params.play_note,
                &p.rec_len,
                &p.rec_rate,
            ][index]
        } else {
            let p = &self.params.play;
            [
                &p.pitch, &p.dec, &p.hold, &p.brr, &p.strt, &p.end, &p.rtrg, &p.rtim,
            ][index]
        }
    }

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
                    let trigger = if record {
                        &self.ui_state.record_trigger
                    } else {
                        &self.ui_state.play_trigger
                    };
                    trigger.store(true, Ordering::Relaxed);
                }
                if record {
                    let p = &self.params.record;
                    let locked = self.ui_state.capture_locked.load(Ordering::Relaxed);
                    let active = self.ui_state.recording.load(Ordering::Relaxed);
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
                                    vertical_slider(ui, self.level(), gui);
                                }
                                1 => {
                                    vertical_slider(ui, self.srr(), gui);
                                }
                                _ => {
                                    vertical_slider(ui, self.distortion(), gui);
                                }
                            }
                            let id = ui.id().with(("sidebar-value", kind));
                            match kind {
                                0 => parameter_value(ui, id, self.level(), gui),
                                1 => parameter_value(ui, id, self.srr(), gui),
                                _ => parameter_value(ui, id, self.distortion(), gui),
                            }
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
                "MAIN",
                "BAL",
                "SIDE",
                "BAL",
                "REC NOTE",
                "PLAY NOTE",
                "LEN",
                "RATE",
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
                    rotary_knob(ui, self.knob(index), gui);
                    parameter_value(ui, ui.id().with(("value", index)), self.knob(index), gui);
                },
            );
        }
    }

    fn status(&self, ui: &mut egui::Ui, len: usize, rate: f32, record: bool) {
        let active = if record {
            &self.ui_state.recording
        } else {
            &self.ui_state.playing
        }
        .load(Ordering::Relaxed);
        let one_shot = record && self.params.record.one_shot.unmodulated_plain_value();
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

    fn footer(&mut self, ui: &mut egui::Ui, gui: &GuiContext, record: bool) {
        ui.horizontal(|ui| {
            let selected = self.params.buffer.unmodulated_plain_value();
            ui.label(format!("▌▌ R{} ▐▐", selected + 1));
            if ui.add_sized([66.0, 28.0], egui::Button::selectable(record, "RECORD")).clicked() {
                self.record_page = true;
                self.meter = 0.0;
            }
            if ui.add_sized([66.0, 28.0], egui::Button::selectable(!record, "PLAY")).clicked() {
                self.record_page = false;
                self.meter = 0.0;
            }
            let only = self.params.playback_only.unmodulated_plain_value();
            if ui
                .add_sized([152.0, 28.0], egui::Button::new(if only { "PLAYBACK ONLY" } else { "DRY + PLAYBACK" }))
                .on_hover_text("Output routing. Playback Only mutes dry-through for capture/send buses. Capture always reads the input before playback.")
                .clicked()
            {
                let p = &self.params.playback_only;
                let setter = gui.param_setter();
                setter.begin_set_parameter(p);
                setter.set_parameter(p, !only);
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
                        let p = &self.params.buffer;
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
