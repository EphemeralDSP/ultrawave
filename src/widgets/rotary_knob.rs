use egui::{self, pos2, vec2, Color32, Response, Sense, Stroke, Ui};
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::Param;

/// Elektron-style rotary encoder. Vertical drag adjusts the value, shift makes
/// the drag ten times finer, and double click restores the default.
pub fn rotary_knob<P: Param>(ui: &mut Ui, param: &P, gui: &GuiContext) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(58.0, 58.0), Sense::click_and_drag());
    super::parameter_drag(ui, &response, param, gui);

    let painter = ui.painter();
    let center = rect.center();
    let radius = 26.0;
    let white = Stroke::new(1.0, Color32::WHITE);
    painter.circle_stroke(center, radius, Stroke::new(2.0, Color32::WHITE));
    for index in 0..11 {
        let angle = (135.0 + index as f32 * 27.0).to_radians();
        let direction = vec2(angle.cos(), angle.sin());
        painter.line_segment(
            [center + direction * 18.0, center + direction * 22.0],
            white,
        );
    }
    let angle = (135.0 + param.unmodulated_normalized_value() * 270.0).to_radians();
    let direction = vec2(angle.cos(), angle.sin());
    painter.line_segment(
        [center + direction * 8.0, center + direction * 22.0],
        Stroke::new(2.0, Color32::WHITE),
    );
    painter.circle_filled(pos2(center.x, center.y), 2.0, Color32::WHITE);
    response
}
