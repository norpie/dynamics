use crate::tui::Element;
use crate::tui::element::FocusId;
use ratatui::style::Style;

/// Builder for button elements
pub struct ButtonBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) label: String,
    pub(crate) on_press: Option<Msg>,
    pub(crate) on_hover: Option<Msg>,
    pub(crate) on_hover_exit: Option<Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
    pub(crate) style: Option<Style>,
}

impl<Msg> ButtonBuilder<Msg> {
    pub fn on_press(mut self, msg: Msg) -> Self {
        self.on_press = Some(msg);
        self
    }

    pub fn on_hover(mut self, msg: Msg) -> Self {
        self.on_hover = Some(msg);
        self
    }

    pub fn on_hover_exit(mut self, msg: Msg) -> Self {
        self.on_hover_exit = Some(msg);
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

    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    pub fn build(self) -> Element<Msg> {
        Element::Button {
            id: self.id,
            label: self.label,
            on_press: self.on_press,
            on_hover: self.on_hover,
            on_hover_exit: self.on_hover_exit,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
            style: self.style,
        }
    }
}
