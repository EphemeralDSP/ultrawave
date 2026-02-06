use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

/// A vertical slider widget with 1-bit monochrome aesthetic
/// Uses block characters to create a filled bar appearance
#[derive(Lens)]
pub struct VerticalSlider {
    param_base: ParamWidgetBase,
    drag_active: bool,
    last_mouse_y: f32,
}

impl VerticalSlider {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
    ) -> Handle<Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            drag_active: false,
            last_mouse_y: 0.0,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |_cx, _param_data| {
                // The slider drawing is handled in the draw() method
            }),
        )
    }
}

impl View for VerticalSlider {
    fn element(&self) -> Option<&'static str> {
        Some("vertical-slider")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.w;
        let h = bounds.h;

        // Get normalized value (0.0 to 1.0)
        let normalized = self.param_base.unmodulated_normalized_value();

        // Draw border (white outline)
        let mut border_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
        border_paint.set_line_width(1.0);
        border_paint.set_anti_alias(false);

        let mut border_path = vg::Path::new();
        border_path.rect(x, y, w, h);
        canvas.stroke_path(&border_path, &border_paint);

        // Draw filled portion (white)
        let fill_height = h * normalized;
        if fill_height > 0.0 {
            let fill_paint = vg::Paint::color(vg::Color::rgb(255, 255, 255));
            let mut fill_path = vg::Path::new();
            fill_path.rect(x + 1.0, y + h - fill_height, w - 2.0, fill_height - 1.0);
            canvas.fill_path(&fill_path, &fill_paint);
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(button) if *button == MouseButton::Left => {
                self.drag_active = true;
                self.last_mouse_y = cx.mouse().cursory;
                self.param_base.begin_set_parameter(cx);
                cx.capture();
                meta.consume();
            }

            WindowEvent::MouseUp(button) if *button == MouseButton::Left => {
                if self.drag_active {
                    self.drag_active = false;
                    self.param_base.end_set_parameter(cx);
                    cx.release();
                    meta.consume();
                }
            }

            WindowEvent::MouseMove(_x, y) => {
                if self.drag_active {
                    // Vertical drag controls the value
                    let sensitivity = 0.005;
                    let delta_y = self.last_mouse_y - *y;
                    self.last_mouse_y = *y;
                    let delta_value = delta_y * sensitivity;
                    let new_value = (self.param_base.unmodulated_normalized_value() + delta_value)
                        .clamp(0.0, 1.0);

                    if (new_value - self.param_base.unmodulated_normalized_value()).abs()
                        > f32::EPSILON
                    {
                        self.param_base.set_normalized_value(cx, new_value);
                        cx.needs_redraw();
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

            _ => {}
        });
    }
}
