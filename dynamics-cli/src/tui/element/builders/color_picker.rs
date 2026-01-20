use crate::tui::Element;
use crate::tui::element::FocusId;
use crate::tui::widgets::{ColorPickerEvent, ColorPickerMode, ColorPickerState};
use ratatui::style::Color;

/// Builder for color picker elements
pub struct ColorPickerBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) value: Color,
    pub(crate) mode: ColorPickerMode,
    pub(crate) state: ColorPickerState,
    pub(crate) on_event: Option<fn(ColorPickerEvent) -> Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
}

impl<Msg> ColorPickerBuilder<Msg> {
    /// Set the event handler
    pub fn on_event(mut self, handler: fn(ColorPickerEvent) -> Msg) -> Self {
        self.on_event = Some(handler);
        self
    }

    /// Set the focus handler
    pub fn on_focus(mut self, msg: Msg) -> Self {
        self.on_focus = Some(msg);
        self
    }

    /// Set the blur handler
    pub fn on_blur(mut self, msg: Msg) -> Self {
        self.on_blur = Some(msg);
        self
    }

    /// Build the color picker element
    pub fn build(self) -> Element<Msg> {
        Element::ColorPicker {
            id: self.id,
            value: self.value,
            mode: self.mode,
            state: self.state,
            on_event: self.on_event,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
        }
    }
}
