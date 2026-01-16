use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::UltrawaveParams;

#[derive(Lens)]
struct EditorData {
    params: Arc<UltrawaveParams>,
}

impl Model for EditorData {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (400, 600))
}

pub fn create(
    params: Arc<UltrawaveParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        EditorData {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            Label::new(cx, "Ultrawave")
                .font_size(24.0)
                .top(Pixels(10.0))
                .bottom(Pixels(20.0));

            nih_plug_vizia::widgets::GenericUi::new(cx, EditorData::params);
        })
        .child_space(Stretch(1.0));
    })
}
