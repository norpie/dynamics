use crate::tui::Element;
use ratatui::style::Style;
use ratatui::text::Line;

/// Builder for styled text elements
pub struct StyledTextBuilder<Msg> {
    pub(crate) line: Line<'static>,
    pub(crate) background: Option<Style>,
    pub(crate) _phantom: std::marker::PhantomData<Msg>,
}

impl<Msg> StyledTextBuilder<Msg> {
    pub fn background(mut self, style: Style) -> Self {
        self.background = Some(style);
        self
    }

    pub fn build(self) -> Element<Msg> {
        Element::StyledText {
            line: self.line,
            background: self.background,
        }
    }
}
