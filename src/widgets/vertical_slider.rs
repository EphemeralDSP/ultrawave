use egui::{vec2, Color32, Response, Sense, Stroke, StrokeKind, Ui};
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::Param;

/// Monochrome vertical fader with the same gesture semantics as the encoders.
pub fn vertical_slider<P: Param>(ui: &mut Ui, param: &P, gui: &GuiContext) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 116.0), Sense::click_and_drag());
    super::parameter_drag(ui, &response, param, gui);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Inside,
    );
    let value = param.unmodulated_normalized_value();
    let mut fill = rect.shrink(1.0);
    fill.set_top(fill.bottom() - fill.height() * value);
    ui.painter().rect_filled(fill, 0.0, Color32::WHITE);
    response
}
