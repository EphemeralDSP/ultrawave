use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

/// When shift+dragging, one pixel corresponds to this much change
const GRANULAR_DRAG_MULTIPLIER: f32 = 0.1;

/// A rotary knob widget matching Elektron hardware style
/// Features:
/// - Circular outline with tick marks
/// - Pointer indicating current value
/// - Mouse drag to change (up/down)
/// - Shift+drag for fine control
/// - Double-click to reset
#[derive(Lens)]
pub struct RotaryKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    granular_drag_status: Option<GranularDragStatus>,
    last_mouse_y: f32,
    knob_bounds: BoundingBox,
}

#[derive(Debug, Clone, Copy)]
pub struct GranularDragStatus {
    starting_y: f32,
    starting_value: f32,
}

impl RotaryKnob {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            drag_active: false,
            granular_drag_status: None,
            last_mouse_y: 0.0,
            knob_bounds: BoundingBox::default(),
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |_cx, _param_data| {
                // The knob drawing is handled in the draw() method
            }),
        )
    }

    /// Convert normalized value to angle in radians.
    /// 0.0 -> bottom-left (135 deg), 0.5 -> top (270 deg / -90 deg), 1.0 -> bottom-right (45 deg)
    fn value_to_angle(normalized: f32) -> f32 {
        let min_angle = 135.0f32.to_radians();
        let max_angle = 405.0f32.to_radians();
        min_angle + normalized * (max_angle - min_angle)
    }
}

impl View for RotaryKnob {
    fn element(&self) -> Option<&'static str> {
        Some("rotary-knob")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.w;
        let h = bounds.h;

        // Use the smaller dimension for the knob size
        let size = w.min(h);
        let center_x = x + w / 2.0;
        let center_y = y + h / 2.0;
        let radius = size / 2.0 - 2.0;

        // Get current normalized value
        let normalized_value = self.param_base.unmodulated_normalized_value();

        // Draw circular outline (white on black for 1-bit monochrome)
        let mut outline_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        outline_paint.set_line_width(2.0);
        outline_paint.set_anti_alias(false);

        let mut outline_path = vg::Path::new();
        outline_path.circle(center_x, center_y, radius);
        canvas.stroke_path(&outline_path, &outline_paint);

        // Draw tick marks across the same active travel arc as the pointer.
        let tick_start_angle = 135.0f32.to_radians();
        let tick_end_angle = 405.0f32.to_radians();
        let num_ticks = 11;
        let tick_outer_radius = radius - 4.0;
        let tick_inner_radius = radius - 8.0;

        let mut tick_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        tick_paint.set_line_width(1.0);
        tick_paint.set_anti_alias(false);

        for i in 0..num_ticks {
            let t = i as f32 / (num_ticks - 1) as f32;
            let angle = tick_start_angle + t * (tick_end_angle - tick_start_angle);

            let x1 = center_x + tick_inner_radius * angle.cos();
            let y1 = center_y + tick_inner_radius * angle.sin();
            let x2 = center_x + tick_outer_radius * angle.cos();
            let y2 = center_y + tick_outer_radius * angle.sin();

            let mut tick_path = vg::Path::new();
            tick_path.move_to(x1, y1);
            tick_path.line_to(x2, y2);
            canvas.stroke_path(&tick_path, &tick_paint);
        }

        // Draw pointer line indicating current value
        let angle = Self::value_to_angle(normalized_value);
        let pointer_inner_radius = radius * 0.3;
        let pointer_outer_radius = radius * 0.85;

        let px1 = center_x + pointer_inner_radius * angle.cos();
        let py1 = center_y + pointer_inner_radius * angle.sin();
        let px2 = center_x + pointer_outer_radius * angle.cos();
        let py2 = center_y + pointer_outer_radius * angle.sin();

        let mut pointer_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        pointer_paint.set_line_width(2.0);
        pointer_paint.set_anti_alias(false);

        let mut pointer_path = vg::Path::new();
        pointer_path.move_to(px1, py1);
        pointer_path.line_to(px2, py2);
        canvas.stroke_path(&pointer_path, &pointer_paint);

        // Draw center dot
        let dot_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        let mut dot_path = vg::Path::new();
        dot_path.circle(center_x, center_y, 2.0);
        canvas.fill_path(&dot_path, &dot_paint);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(button) if *button == MouseButton::Left => {
                let cursory = cx.mouse().cursory;
                // Check if click is within our bounds by checking parent layout
                // For now, accept all clicks when not dragging and let the capture handle it
                if cx.modifiers().alt() {
                    // Alt+click initiates text input (optional feature)
                } else {
                    self.drag_active = true;
                    self.last_mouse_y = cursory;

                    if cx.modifiers().shift() {
                        self.granular_drag_status = Some(GranularDragStatus {
                            starting_y: cursory,
                            starting_value: self.param_base.unmodulated_normalized_value(),
                        });
                    }

                    self.param_base.begin_set_parameter(cx);
                    cx.capture();
                    meta.consume();
                }
            }

            WindowEvent::MouseUp(button) if *button == MouseButton::Left => {
                if self.drag_active {
                    self.drag_active = false;
                    self.granular_drag_status = None;
                    self.param_base.end_set_parameter(cx);
                    cx.release();
                    meta.consume();
                }
            }

            WindowEvent::MouseMove(_x, y) => {
                if self.drag_active {
                    if let Some(granular) = &self.granular_drag_status {
                        let delta_y = granular.starting_y - *y;
                        let delta_value = delta_y * GRANULAR_DRAG_MULTIPLIER * 0.01;
                        let new_value = (granular.starting_value + delta_value).clamp(0.0, 1.0);

                        if (new_value - self.param_base.unmodulated_normalized_value()).abs()
                            > f32::EPSILON
                        {
                            self.param_base.set_normalized_value(cx, new_value);
                            cx.needs_redraw();
                        }
                    } else {
                        // Regular drag - use vertical movement
                        let sensitivity = 0.005;
                        let delta_y = self.last_mouse_y - *y;
                        self.last_mouse_y = *y;
                        let delta_value = delta_y * sensitivity;
                        let new_value = (self.param_base.unmodulated_normalized_value()
                            + delta_value)
                            .clamp(0.0, 1.0);

                        if (new_value - self.param_base.unmodulated_normalized_value()).abs()
                            > f32::EPSILON
                        {
                            self.param_base.set_normalized_value(cx, new_value);
                            cx.needs_redraw();
                        }
                    }
                }
            }

            WindowEvent::MouseDoubleClick(button) if *button == MouseButton::Left => {
                self.param_base.begin_set_parameter(cx);
                self.param_base
                    .set_normalized_value(cx, self.param_base.default_normalized_value());
                self.param_base.end_set_parameter(cx);
                cx.needs_redraw();
                meta.consume();
            }

            WindowEvent::KeyDown(code, _) => {
                if (*code == Code::ShiftLeft || *code == Code::ShiftRight) && self.drag_active {
                    self.granular_drag_status = Some(GranularDragStatus {
                        starting_y: cx.mouse().cursory,
                        starting_value: self.param_base.unmodulated_normalized_value(),
                    });
                }
            }

            WindowEvent::KeyUp(code, _)
                if (*code == Code::ShiftLeft || *code == Code::ShiftRight) =>
            {
                self.granular_drag_status = None;
            }

            _ => {}
        });
    }
}
