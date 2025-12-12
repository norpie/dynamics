use crate::tui::Element;
use crate::tui::element::FocusId;

/// Builder for checkbox elements
pub struct CheckboxBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) label: String,
    pub(crate) checked: bool,
    pub(crate) on_toggle: Option<Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
}

impl<Msg> CheckboxBuilder<Msg> {
    pub fn on_toggle(mut self, msg: Msg) -> Self {
        self.on_toggle = Some(msg);
        self
    }

    pub fn on_focus(mut self, msg: Msg) -> Self {
        self.on_focus = Some(msg);
        self
    }

    pub fn on_blur(mut self, msg: Msg) -> Self {
        self.on_blur = Some(msg);
        self
    }

    pub fn build(self) -> Element<Msg> {
        Element::Checkbox {
            id: self.id,
            label: self.label,
            checked: self.checked,
            on_toggle: self.on_toggle,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
        }
    }
}
