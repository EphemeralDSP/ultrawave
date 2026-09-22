use egui::{
    self,
    containers::menu::{MenuButton, MenuConfig},
    vec2, Color32, FontData, FontDefinitions, FontFamily, FontId, Layout, RichText, Stroke,
    StrokeKind, UiBuilder,
};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::LogicalSize;
use nice_plug::prelude::{BoolParam, IntParam, Param};
use nice_plug_egui::{
    create_egui_editor, EguiEditor, EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp,
    RepaintNotifier,
};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use super::UiState;
use crate::machines::memory::SampleMemory;
use crate::params::UltrawaveParams;
use crate::widgets::{parameter_value, rotary_knob, waveform, WaveformCache};

const BACKGROUND: Color32 = Color32::from_rgb(19, 21, 24);
const PANEL: Color32 = Color32::from_rgb(28, 31, 35);
const BORDER: Color32 = Color32::from_rgb(58, 63, 70);
const CAPTURE: Color32 = Color32::from_rgb(237, 175, 128);
const PLAY: Color32 = Color32::from_rgb(183, 208, 222);

pub struct UltrawaveEditorApp {
    params: Arc<UltrawaveParams>,
    midi_open: bool,
    ui_state: Arc<UiState>,
    gui: Option<GuiContext>,
    waveform: Vec<i16>,
    metadata: Option<(usize, u64)>,
    waveform_cache: WaveformCache,
    meters: [f32; 2],
    memory: Arc<SampleMemory>,
}

pub type PluginEditor = EguiEditor<UltrawaveEditorApp>;

pub fn default_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(LogicalSize::new(920.0, 650.0), 1.0)
}

pub fn create(
    params: Arc<UltrawaveParams>,
    state: Arc<EguiEditorState>,
    ui: Arc<UiState>,
    memory: Arc<SampleMemory>,
) -> Option<PluginEditor> {
    create_egui_editor(
        state,
        RepaintNotifier::new(),
        EguiNiceSettings::new().with_tile("Ultrawave"),
        UltrawaveEditorApp {
            params,
            midi_open: false,
            ui_state: ui,
            gui: None,
            waveform: Vec::new(),
            metadata: None,
            waveform_cache: WaveformCache::default(),
            meters: [0.0; 2],
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
        for (name, family, bytes) in [
            (
                "uw-sans",
                FontFamily::Proportional,
                include_bytes!("../../assets/fonts/DejaVuSans.ttf").as_slice(),
            ),
            (
                "uw-mono",
                FontFamily::Monospace,
                include_bytes!("../../assets/fonts/DejaVuSansMono.ttf").as_slice(),
            ),
        ] {
            fonts
                .font_data
                .insert(name.into(), Arc::new(FontData::from_static(bytes)));
            fonts.families.insert(family, vec![name.into()]);
        }
        ctx.set_fonts(fonts);
        ctx.set_theme(egui::Theme::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = PANEL;
        visuals.weak_text_color = Some(Color32::from_gray(160));
        visuals.widgets.inactive.bg_fill = PANEL;
        visuals.widgets.inactive.weak_bg_fill = PANEL;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, PLAY);
        visuals.selection.bg_fill = Color32::from_rgb(57, 75, 85);
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
        ] {
            widget.corner_radius = 4.into();
        }
        ctx.set_visuals(visuals);
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.override_font_id = Some(FontId::proportional(13.0));
            style.spacing.button_padding = vec2(10.0, 6.0);
        });
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
        let Some(gui) = self.gui.clone() else { return };
        let snapshot = self.memory.snapshot();
        let (len, rate, revision) = snapshot.unwrap_or((0, 44_100.0, 0));
        let metadata = Some((len, revision));
        if snapshot.is_none() {
            self.waveform.clear();
            self.metadata = None;
        } else if metadata != self.metadata {
            if let Some(samples) = self.memory.read_buffer() {
                self.waveform = samples;
                self.metadata = metadata;
            } else {
                self.waveform.clear();
                self.metadata = None;
            }
        }
        for (meter, source) in self
            .meters
            .iter_mut()
            .zip([&self.ui_state.capture_peak, &self.ui_state.playback_peak])
        {
            let peak = f32::from_bits(source.swap(0, Ordering::Relaxed));
            let db = 20.0 * peak.max(0.000_001).log10();
            *meter = ((db + 60.0) / 60.0).clamp(0.0, 1.0).max(*meter * 0.75);
        }
        let origin = ui.max_rect().min;
        let rect = |x, y, w, h| egui::Rect::from_min_size(origin + vec2(x, y), vec2(w, h));
        ui.painter().rect_filled(ui.max_rect(), 0.0, BACKGROUND);
        ui.scope_builder(UiBuilder::new().max_rect(rect(24.0, 18.0, 872.0, 48.0)), |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("ULTRAWAVE").font(FontId::monospace(24.0)).strong());
                    ui.label(RichText::new("12-bit capture sampler").weak());
                });
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.midi_open, "MIDI ▾").clicked() {
                        self.midi_open = !self.midi_open;
                    }
                    ui.add_space(18.0);
                    let only = self.params.playback_only.unmodulated_plain_value();
                    for (value, label) in [(true, "Sample only"), (false, "Input + sample")] {
                        if ui.selectable_label(only == value, label).on_hover_text(
                            "Output only. Capture always reads the input before sample playback is added."
                        ).clicked() {
                            set_bool(&gui, &self.params.playback_only, value);
                        }
                    }
                    ui.label(RichText::new("OUTPUT").small().weak());
                });
            });
        });
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(24.0, 74.0, 872.0, 20.0)),
            |ui| {
                ui.label(
                    RichText::new("Capture audio  →  shape your take  →  play it back").weak(),
                );
            },
        );
        let wave_rect = rect(24.0, 102.0, 872.0, 100.0);
        waveform(
            ui,
            wave_rect,
            &self.waveform,
            (Arc::as_ptr(&self.memory) as usize, len, revision),
            &mut self.waveform_cache,
            Some((&self.params.play.strt, &self.params.play.end)),
            self.ui_state.playback_position(len),
            &gui,
        );
        if len == 0 {
            ui.painter()
                .rect_filled(wave_rect.shrink(10.0), 0.0, Color32::BLACK);
            ui.painter().text(
                wave_rect.center() - vec2(0.0, 12.0),
                egui::Align2::CENTER_CENTER,
                "No sample yet",
                FontId::proportional(18.0),
                Color32::WHITE,
            );
            ui.painter().text(
                wave_rect.center() + vec2(0.0, 15.0),
                egui::Align2::CENTER_CENTER,
                "Play audio through this track, then press Capture.",
                FontId::proportional(13.0),
                PLAY,
            );
        }
        ui.scope_builder(UiBuilder::new().max_rect(rect(24.0, 211.0, 872.0, 24.0)), |ui| {
            ui.horizontal(|ui| {
                for (label, p) in [("Start", &self.params.play.strt), ("End", &self.params.play.end)] {
                    ui.label(RichText::new(label).weak());
                    parameter_value(ui, ui.id().with(label), p, &gui);
                    ui.add_space(12.0);
                }
                let start = self.params.play.strt.unmodulated_plain_value();
                let end = self.params.play.end.unmodulated_plain_value();
                ui.label(RichText::new(if start > end { "← REVERSE" } else if start == end { "EMPTY REGION" } else { "FORWARD →" }).color(PLAY))
                    .on_hover_text("Drag the Start handle past End to reverse the sample. Direction latches when playback starts.");
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.2} s  ·  {:.1} kHz  ·  12-bit mono", len as f32 / rate.max(1.0), rate / 1000.0)).monospace().small().weak());
                });
            });
        });
        for (area, color) in [
            (rect(24.0, 250.0, 288.0, 354.0), CAPTURE),
            (rect(328.0, 250.0, 568.0, 354.0), PLAY),
        ] {
            ui.painter().rect(
                area,
                5.0,
                PANEL,
                Stroke::new(1.0, BORDER),
                StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [
                    area.left_top() + vec2(16.0, 0.0),
                    area.left_top() + vec2(65.0, 0.0),
                ],
                Stroke::new(2.0, color),
            );
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(40.0, 266.0, 256.0, 320.0)),
            |ui| self.capture_panel(ui, &gui),
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(rect(344.0, 266.0, 536.0, 320.0)),
            |ui| self.play_panel(ui, &gui, len),
        );
        for (index, (x, width, color)) in [(40.0, 256.0, CAPTURE), (344.0, 536.0, PLAY)]
            .into_iter()
            .enumerate()
        {
            let meter = rect(x, 594.0, width, 3.0);
            ui.painter().rect_filled(meter, 0.0, BORDER);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(meter.min, vec2(width * self.meters[index], 3.0)),
                0.0,
                color,
            );
        }
        ui.scope_builder(UiBuilder::new().max_rect(rect(24.0, 621.0, 872.0, 20.0)), |ui| {
            ui.label(RichText::new("One instance · one take. New captures replace it. Audio is not saved with the project.").small().weak());
        });
        // A settings window, rather than a menu: ComboBox popups use egui's
        // popup stack and otherwise dismiss their parent menu when opening.
        let mut open = self.midi_open;
        egui::Window::new("MIDI triggers")
            .open(&mut open)
            .anchor(egui::Align2::RIGHT_TOP, vec2(-24.0, 62.0))
            .default_width(252.0)
            .resizable(false)
            .collapsible(false)
            .show(ui.ctx(), |ui| self.midi_panel(ui, &gui));
        self.midi_open = open;
    }

    fn editor_closed(&mut self) {
        self.gui = None;
    }
}

impl UltrawaveEditorApp {
    fn midi_panel(&self, ui: &mut egui::Ui, gui: &GuiContext) {
        ui.set_width(252.0);
        for (label, param) in [
            ("Capture", &self.params.record_note),
            ("Play sample", &self.params.play_note),
        ] {
            ui.horizontal(|ui| {
                ui.add_sized([85.0, 24.0], egui::Label::new(label));
                int_choice(
                    ui,
                    param,
                    gui,
                    132.0,
                    (0..=127).map(|note| (note, midi_note(note))),
                );
            });
        }
        ui.label(
            RichText::new(
                "MIDI numbers are definitive; DAW octave names vary. Other notes are ignored.",
            )
            .small()
            .weak(),
        );
        if self.params.record_note.unmodulated_plain_value()
            == self.params.play_note.unmodulated_plain_value()
        {
            ui.label(
                RichText::new(
                    "Same note: Capture takes priority. Choose different notes to play back.",
                )
                .color(CAPTURE),
            );
        }
        ui.separator();
        ui.label(RichText::new("Renoise: send notes through an FX alias to this effect. Buttons work without MIDI.").small());
    }

    fn capture_panel(&self, ui: &mut egui::Ui, gui: &GuiContext) {
        let p = &self.params.record;
        let recording = self.ui_state.recording.load(Ordering::Relaxed);
        let once = p.one_shot.unmodulated_plain_value();
        let locked = once && self.ui_state.capture_locked.load(Ordering::Relaxed);
        ui.label(RichText::new("01  CAPTURE").color(CAPTURE).strong());
        ui.label(
            RichText::new("Record the audio arriving at this effect.")
                .small()
                .weak(),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!locked, egui::Button::new(RichText::new("●  Capture").color(BACKGROUND).strong()).fill(CAPTURE).min_size(vec2(133.0, 34.0)))
                .on_hover_text("Start a timed capture. This replaces the previous take and stops its playback.").clicked() {
                self.ui_state.record_trigger.store(true, Ordering::Relaxed);
            }
            int_choice(ui, &p.rec_len, gui, 96.0, [(0, "1/2 beat"), (16, "1 beat"), (32, "2 beats"), (48, "1 bar"), (64, "2 bars"), (80, "4 bars"), (96, "8 bars"), (112, "Maximum")].into_iter().map(|(v, s)| (v, s.into())));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut checked = once;
            if ui.checkbox(&mut checked, "Record once").on_hover_text("Protect the first take from later capture triggers, even when the pattern loops.").changed() {
                set_bool(gui, &p.one_shot, checked);
            }
            if ui.add_enabled(locked && !recording, egui::Button::new("Re-arm"))
                .on_hover_text("Keep this take until the next Capture trigger replaces it.").clicked() {
                self.ui_state.rearm.store(true, Ordering::Relaxed);
            }
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(if recording {
                "● RECORDING"
            } else if locked {
                "SAMPLE PROTECTED"
            } else {
                "READY TO CAPTURE"
            })
            .color(CAPTURE)
            .small()
            .strong(),
        );
        ui.label(
            RichText::new(if recording {
                "The take ends at the selected duration."
            } else if locked {
                "Re-arm to allow another take."
            } else {
                "The next trigger replaces the sample."
            })
            .small()
            .weak(),
        );

        let origin = ui.max_rect().min;
        for (index, (label, param, hint)) in [
            (
                "Track input",
                &p.mlev,
                "Audio from this track or bus, before sample playback is added.",
            ),
            (
                "Sidechain",
                &p.ilev,
                "Audio sent to the effect's optional sidechain. Route it in your DAW first.",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            knob(
                ui,
                origin + vec2(index as f32 * 86.0, 181.0),
                84.0,
                label,
                param,
                gui,
                hint,
            );
        }
        knob(ui, origin + vec2(172.0, 181.0), 84.0, "Gain", &p.lev, gui, "Level into the 12-bit recorder. Hot signals clip; quiet signals lose detail. Dry audio stays unchanged.");
        ui.scope_builder(
            UiBuilder::new().max_rect(egui::Rect::from_min_size(
                origin + vec2(0.0, 283.0),
                vec2(256.0, 30.0),
            )),
            |ui| {
                MenuButton::new("Capture sound & balance ▾")
                    .config(
                        MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| {
                        ui.set_width(376.0);
                        ui.label(RichText::new("PRINT INTO THE TAKE").strong());
                        ui.label(
                            RichText::new("These controls affect recording, never dry-through.")
                                .small()
                                .weak(),
                        );
                        ui.horizontal(|ui| {
                            for (label, param) in [
                                ("Track balance", &p.mbal),
                                ("Side balance", &p.ibal),
                                ("Capture rate", &p.rec_rate),
                            ] {
                                knob_column(ui, 118.0, label, param, gui);
                            }
                        });
                        ui.horizontal(|ui| {
                            knob_column(ui, 118.0, "Rate crush", &p.srr, gui);
                            knob_column(ui, 118.0, "Drive", &p.dist, gui);
                        });
                    });
            },
        );
    }

    fn play_panel(&self, ui: &mut egui::Ui, gui: &GuiContext, len: usize) {
        let p = &self.params.play;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("02  PLAY SAMPLE").color(PLAY).strong());
                ui.label(RichText::new(if self.ui_state.playing.load(Ordering::Relaxed) { "● PLAYING" } else if len == 0 { "Capture a take to begin." } else { "Your take is ready to play." }).small().weak());
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(len > 0, egui::Button::new(RichText::new("▶  Play sample").color(BACKGROUND).strong()).fill(PLAY).min_size(vec2(142.0, 34.0)))
                    .on_hover_text("Trigger the sample from Start. Pitch sets playback speed; MIDI velocity sets loudness.").clicked() {
                    self.ui_state.play_trigger.store(true, Ordering::Relaxed);
                }
            });
        });
        let origin = ui.max_rect().min;
        for (x, y, text) in [
            (0.0, 64.0, "PITCH & ENVELOPE"),
            (338.0, 64.0, "REPEAT"),
            (0.0, 199.0, "CHARACTER"),
            (432.0, 199.0, "OUTPUT"),
        ] {
            ui.painter().text(
                origin + vec2(x, y),
                egui::Align2::LEFT_TOP,
                text,
                FontId::proportional(11.0),
                PLAY,
            );
        }
        for (x, label, param, hint) in [
            (0.0, "Pitch", &p.pitch, "Varispeed in semitones. Independent of MIDI note number."),
            (106.0, "Hold", &p.hold, "Time at full level before decay. Hold and Decay both zero bypass the envelope."),
            (212.0, "Decay", &p.dec, "Fade-out after Hold."),
            (338.0, "Repeats", &p.rtrg, "Extra repeats after the first hit. INF repeats until stopped. Interval must also be nonzero."),
            (444.0, "Interval", &p.rtim, "Tempo-synced time between hits. OFF disables repeats."),
        ] {
            knob(ui, origin + vec2(x, 84.0), 90.0, label, param, gui, hint);
        }
        knob(
            ui,
            origin + vec2(0.0, 220.0),
            90.0,
            "Rate crush",
            &p.srr,
            gui,
            "Sample-and-hold reduction adds stepped, aliased texture.",
        );
        knob(
            ui,
            origin + vec2(106.0, 220.0),
            90.0,
            "Bit crush",
            &p.brr,
            gui,
            "Remove precision beyond the base 12-bit recording, down to 2 bits.",
        );
        knob(
            ui,
            origin + vec2(212.0, 220.0),
            90.0,
            "Drive",
            &p.dist,
            gui,
            "Distortion on playback only.",
        );
        knob(
            ui,
            origin + vec2(444.0, 220.0),
            90.0,
            "Level",
            &p.lev,
            gui,
            "Sample output level. Does not change dry-through.",
        );
    }
}

fn set_bool(gui: &GuiContext, param: &BoolParam, value: bool) {
    let setter = gui.param_setter();
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn midi_note(note: i32) -> String {
    let names = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!("{note} · {}{}", names[note as usize % 12], note / 12 - 1)
}

fn int_choice(
    ui: &mut egui::Ui,
    param: &IntParam,
    gui: &GuiContext,
    width: f32,
    choices: impl Iterator<Item = (i32, String)>,
) {
    let current = param.unmodulated_plain_value();
    let options: Vec<_> = choices.collect();
    // Capture lengths are ranges of the hardware's 0–127 parameter. Select the
    // last landmark not above the value, including when the host automates it.
    let selected = options.iter().rev().find(|(v, _)| *v <= current);
    let open_id = ui.id().with((param.name(), "open"));
    let was_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let mut open = false;
    egui::ComboBox::from_id_salt(param.name())
        .width(width)
        .height(230.0)
        .selected_text(selected.map_or("", |(_, label)| label.as_str()))
        .show_ui(ui, |ui| {
            open = true;
            for (value, label) in &options {
                let chosen = selected.is_some_and(|(v, _)| v == value);
                let response = ui.selectable_label(chosen, label);
                if chosen && !was_open {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    let setter = gui.param_setter();
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, *value);
                    setter.end_set_parameter(param);
                }
            }
        });
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

#[allow(clippy::too_many_arguments)]
fn knob<P: Param>(
    ui: &mut egui::Ui,
    position: egui::Pos2,
    width: f32,
    label: &str,
    param: &P,
    gui: &GuiContext,
    hint: &str,
) {
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(position, vec2(width, 100.0)))
            .layout(Layout::top_down(egui::Align::Center)),
        |ui| {
            ui.label(RichText::new(label).small()).on_hover_text(hint);
            rotary_knob(ui, param, gui).on_hover_text(hint);
            parameter_value(ui, ui.id().with(label), param, gui);
        },
    );
}

fn knob_column<P: Param>(ui: &mut egui::Ui, width: f32, label: &str, param: &P, gui: &GuiContext) {
    ui.allocate_ui_with_layout(
        vec2(width, 100.0),
        Layout::top_down(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(label).small());
            rotary_knob(ui, param, gui);
            parameter_value(ui, ui.id().with(label), param, gui);
        },
    );
}
