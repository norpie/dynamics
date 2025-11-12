use ratatui::style::Style;
use ratatui::text::Line;

// Import builder types
mod builders;
pub use builders::*;

/// Stable identifier for focusable UI elements
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusId(pub &'static str);

impl FocusId {
    /// Create a new FocusId with a static string identifier
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

impl From<&'static str> for FocusId {
    fn from(s: &'static str) -> Self {
        FocusId(s)
    }
}

/// Alignment options for positioned elements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Center,
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// A layer in a stack of UI elements
#[derive(Clone)]
pub struct Layer<Msg> {
    pub element: Element<Msg>,
    pub alignment: Alignment,
    pub dim_below: bool,
}

impl<Msg> Layer<Msg> {
    pub fn new(element: Element<Msg>) -> Self {
        Self {
            element,
            alignment: Alignment::TopLeft,
            dim_below: false,
        }
    }

    pub fn center(mut self) -> Self {
        self.alignment = Alignment::Center;
        self
    }

    pub fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn dim(mut self, should_dim: bool) -> Self {
        self.dim_below = should_dim;
        self
    }
}

/// Layout constraints for sizing elements within containers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutConstraint {
    /// Fixed size (exact number of lines/columns)
    Length(u16),
    /// Minimum size (at least this many lines/columns)
    Min(u16),
    /// Proportional fill (weight for distributing remaining space)
    Fill(u16),
}

/// Declarative UI elements that compose to form the view
#[derive(Clone)]
pub enum Element<Msg> {
    /// Empty element that renders nothing
    None,

    /// Static text
    Text { content: String, style: Option<Style> },

    /// Styled text with multiple spans
    StyledText {
        line: Line<'static>,
        background: Option<Style>,
    },

    /// Interactive button
    Button {
        id: FocusId,
        label: String,
        on_press: Option<Msg>,
        on_hover: Option<Msg>,
        on_hover_exit: Option<Msg>,
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
        style: Option<Style>,
    },

    /// Vertical layout container
    Column {
        items: Vec<(LayoutConstraint, Element<Msg>)>,
        spacing: u16,
    },

    /// Horizontal layout container
    Row {
        items: Vec<(LayoutConstraint, Element<Msg>)>,
        spacing: u16,
    },

    /// Container with padding/margins
    Container {
        child: Box<Element<Msg>>,
        padding: u16,
    },

    /// Panel with border
    Panel {
        child: Box<Element<Msg>>,
        title: Option<String>,
        width: Option<u16>,
        height: Option<u16>,
    },

    /// Stack of layered elements (for modals, overlays)
    Stack {
        layers: Vec<Layer<Msg>>,
    },

    /// Scrollable list of items
    List {
        id: FocusId,
        items: Vec<Element<Msg>>,
        selected: Option<usize>,
        scroll_offset: usize,
        on_select: Option<fn(usize) -> Msg>,
        on_activate: Option<fn(usize) -> Msg>,
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
        on_render: Option<fn(usize) -> Msg>,  // Called with actual viewport height from renderer
    },

    /// Single-line text input
    TextInput {
        id: FocusId,
        value: String,
        cursor_pos: usize,
        scroll_offset: usize,
        placeholder: Option<String>,
        max_length: Option<usize>,
        masked: bool,
        on_change: Option<fn(crossterm::event::KeyCode) -> Msg>,
        on_submit: Option<Msg>,
        on_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,  // Unified event handler
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// Hierarchical tree with expand/collapse
    Tree {
        id: FocusId,
        items: Vec<Element<Msg>>,       // Flattened nodes
        node_ids: Vec<String>,           // Parallel array of node IDs
        selected: Option<String>,        // Selected node ID (not index!)
        scroll_offset: usize,
        on_select: Option<fn(String) -> Msg>,     // ID-based callbacks
        on_toggle: Option<fn(String) -> Msg>,     // Expand/collapse callback
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
        on_event: Option<fn(crate::tui::widgets::TreeEvent) -> Msg>,  // Unified event pattern
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
        on_render: Option<fn(usize) -> Msg>,  // Called with actual viewport height from renderer
    },

    /// Table-style tree with columns and borders
    TableTree {
        id: FocusId,
        flattened_nodes: Vec<crate::tui::widgets::FlatTableNode>,  // Pre-flattened table nodes
        node_ids: Vec<String>,           // Parallel array of node IDs
        selected: Option<String>,        // Selected node ID (not index!)
        scroll_offset: usize,
        column_widths: Vec<ratatui::layout::Constraint>,  // Column layout constraints
        column_headers: Vec<String>,     // Column header labels
        on_select: Option<fn(String) -> Msg>,     // ID-based callbacks
        on_event: Option<fn(crate::tui::widgets::TreeEvent) -> Msg>,  // Unified event pattern
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
        on_render: Option<fn(usize) -> Msg>,  // Called with actual viewport height from renderer
    },

    /// Scrollable wrapper for any element
    Scrollable {
        id: FocusId,
        child: Box<Element<Msg>>,
        scroll_offset: usize,
        content_height: Option<usize>,   // If None, auto-detect from Column
        horizontal_scroll_offset: usize,
        content_width: Option<usize>,    // If None, auto-detect
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
        on_render: Option<fn(usize, usize, usize, usize) -> Msg>,  // (viewport_height, content_height, viewport_width, content_width)
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// Select/Dropdown widget
    Select {
        id: FocusId,
        options: Vec<String>,               // Display labels for options
        selected: usize,                    // Selected index
        is_open: bool,                      // Dropdown open?
        highlight: usize,                   // Highlighted option (when open)
        on_select: Option<fn(usize) -> Msg>,  // Called when option selected
        on_toggle: Option<Msg>,             // Called when dropdown toggled
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,  // Called for keyboard navigation when open
        on_event: Option<fn(crate::tui::widgets::SelectEvent) -> Msg>,  // Unified event handler
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// Autocomplete input with fuzzy-matched dropdown
    Autocomplete {
        id: FocusId,
        all_options: Vec<String>,           // Full list to filter against
        current_input: String,              // Current input text
        placeholder: Option<String>,        // Placeholder text when empty
        is_open: bool,                      // Dropdown open?
        filtered_options: Vec<String>,      // Filtered options (top 15)
        highlight: usize,                   // Highlighted index in dropdown
        on_input: Option<fn(crossterm::event::KeyCode) -> Msg>,  // Text input changes
        on_select: Option<fn(String) -> Msg>,  // Option selected from dropdown
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,  // Dropdown navigation
        on_event: Option<fn(crate::tui::widgets::AutocompleteEvent) -> Msg>,  // Unified event handler
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// Multi-select input with fuzzy-matched dropdown and chips for selected items
    MultiSelect {
        id: FocusId,
        all_options: Vec<String>,           // Full list to filter against
        selected_items: Vec<String>,        // Currently selected items
        search_input: String,               // Current search text
        placeholder: Option<String>,        // Placeholder text when empty
        is_open: bool,                      // Dropdown open?
        filtered_options: Vec<String>,      // Filtered options (top 15)
        highlight: usize,                   // Highlighted index in dropdown
        on_event: Option<fn(crate::tui::widgets::MultiSelectEvent) -> Msg>,  // Unified event handler
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// File browser widget
    FileBrowser {
        id: FocusId,
        current_path: std::path::PathBuf,
        entries: Vec<Element<Msg>>,         // Pre-rendered entries as list items
        selected: Option<usize>,
        scroll_offset: usize,
        on_file_selected: Option<fn(std::path::PathBuf) -> Msg>,
        on_directory_changed: Option<fn(std::path::PathBuf) -> Msg>,
        on_directory_entered: Option<fn(std::path::PathBuf) -> Msg>,
        on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
        on_event: Option<fn(crate::tui::widgets::FileBrowserEvent) -> Msg>,
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
        on_render: Option<fn(usize) -> Msg>,  // Called with actual viewport height from renderer
    },

    /// Color picker widget (HSL/RGB sliders + hex input)
    ColorPicker {
        id: FocusId,
        value: ratatui::style::Color,                       // Current color
        mode: crate::tui::widgets::ColorPickerMode,         // HSL or RGB mode
        state: crate::tui::widgets::ColorPickerState,       // Widget state
        on_event: Option<fn(crate::tui::widgets::ColorPickerEvent) -> Msg>,  // Unified event handler
        on_focus: Option<Msg>,
        on_blur: Option<Msg>,
    },

    /// Progress bar showing completion (non-interactive)
    ProgressBar {
        current: usize,
        total: usize,
        label: Option<String>,
        show_percentage: bool,
        show_count: bool,
        width: Option<u16>,
    },
}

impl<Msg> Element<Msg> {
    /// Create a text element
    pub fn text(content: impl Into<String>) -> Self {
        Element::Text {
            content: content.into(),
            style: None,
        }
    }

    /// Create a styled text element with optional background fill
    pub fn styled_text(line: Line<'static>) -> StyledTextBuilder<Msg> {
        StyledTextBuilder {
            line,
            background: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a progress bar element
    pub fn progress_bar(current: usize, total: usize) -> ProgressBarBuilder<Msg> {
        ProgressBarBuilder {
            current,
            total,
            label: None,
            show_percentage: true,
            show_count: true,
            width: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a button element
    pub fn button(id: impl Into<FocusId>, label: impl Into<String>) -> ButtonBuilder<Msg> {
        ButtonBuilder {
            id: id.into(),
            label: label.into(),
            on_press: None,
            on_hover: None,
            on_hover_exit: None,
            on_focus: None,
            on_blur: None,
            style: None,
        }
    }

    /// Create a column layout (old API - backward compatible)
    pub fn column(children: Vec<Element<Msg>>) -> ColumnBuilder<Msg> {
        // Convert children to items with default constraints
        let items = children
            .into_iter()
            .map(|child| (child.default_constraint(), child))
            .collect();

        ColumnBuilder {
            items,
            spacing: 1,
        }
    }

    /// Create a row layout (old API - backward compatible)
    pub fn row(children: Vec<Element<Msg>>) -> RowBuilder<Msg> {
        // Convert children to items with default constraints
        let items = children
            .into_iter()
            .map(|child| (child.default_constraint(), child))
            .collect();

        RowBuilder {
            items,
            spacing: 1,
        }
    }

    /// Wrap element in a container
    pub fn container(child: Element<Msg>) -> ContainerBuilder<Msg> {
        ContainerBuilder {
            child: Box::new(child),
            padding: 1,
        }
    }

    /// Wrap element in a panel with border
    pub fn panel(child: Element<Msg>) -> PanelBuilder<Msg> {
        PanelBuilder {
            child: Box::new(child),
            title: None,
            width: None,
            height: None,
        }
    }

    /// Create a stack of layers
    pub fn stack(layers: Vec<Layer<Msg>>) -> Self {
        Element::Stack { layers }
    }

    /// Create a confirmation modal overlay
    pub fn modal_confirm(
        background: Element<Msg>,
        title: impl Into<String>,
        message: impl Into<String>,
        cancel_id: impl Into<FocusId>,
        on_cancel: Msg,
        confirm_id: impl Into<FocusId>,
        on_confirm: Msg,
    ) -> Self {
        use crate::tui::element::RowBuilder;

        let button_row = RowBuilder::new()
            .add(
                Element::button(cancel_id, "Cancel").on_press(on_cancel).build(),
                LayoutConstraint::Fill(1),
            )
            .add(
                Element::text("  "),
                LayoutConstraint::Length(2),
            )
            .add(
                Element::button(confirm_id, "Confirm").on_press(on_confirm).build(),
                LayoutConstraint::Fill(1),
            )
            .spacing(0)
            .build();

        let modal_content = ColumnBuilder::new()
            .add(
                Element::text(title.into()),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text(""),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text(message.into()),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text(""),
                LayoutConstraint::Length(1),
            )
            .add(
                button_row,
                LayoutConstraint::Length(3),
            )
            .spacing(0)
            .build();

        Element::stack(vec![
            Layer::new(background),
            Layer::new(
                Element::panel(
                    Element::container(modal_content)
                        .padding(1)
                        .build()
                )
                .title("Confirmation")
                .width(60)
                .height(15)
                .build()
            ).center(),
        ])
    }

    /// Get the default layout constraint for this element type
    pub fn default_constraint(&self) -> LayoutConstraint {
        match self {
            Element::None => LayoutConstraint::Length(0),
            Element::Text { .. } => LayoutConstraint::Length(1),
            Element::StyledText { .. } => LayoutConstraint::Length(1),
            Element::Button { .. } => LayoutConstraint::Length(3),
            Element::Column { .. } => LayoutConstraint::Fill(1),
            Element::Row { .. } => LayoutConstraint::Fill(1),
            Element::Container { .. } => LayoutConstraint::Fill(1),
            Element::Panel { child, height, .. } => {
                // If explicit height is set, use it
                if let Some(h) = height {
                    LayoutConstraint::Length(*h)
                } else {
                    // Panel sizes to child + 2 lines for borders (top + bottom)
                    match child.default_constraint() {
                        LayoutConstraint::Length(n) => LayoutConstraint::Length(n + 2),
                        LayoutConstraint::Min(n) => LayoutConstraint::Min(n + 2),
                        LayoutConstraint::Fill(w) => LayoutConstraint::Fill(w),
                    }
                }
            }
            Element::Stack { .. } => LayoutConstraint::Fill(1),
            Element::List { .. } => LayoutConstraint::Fill(1),
            Element::TextInput { .. } => LayoutConstraint::Length(1),
            Element::Tree { .. } => LayoutConstraint::Fill(1),
            Element::TableTree { .. } => LayoutConstraint::Fill(1),
            Element::Scrollable { .. } => LayoutConstraint::Fill(1),
            Element::Select { .. } => LayoutConstraint::Length(1),  // Borderless like TextInput
            Element::Autocomplete { .. } => LayoutConstraint::Length(1),  // Borderless like TextInput
            Element::MultiSelect { .. } => LayoutConstraint::Length(3),  // Chips + input + dropdown preview
            Element::FileBrowser { .. } => LayoutConstraint::Fill(1),  // Fill available space like List
            Element::ColorPicker { .. } => LayoutConstraint::Length(9),  // 3 sliders + hex + labels
            Element::ProgressBar { .. } => LayoutConstraint::Length(1),  // Single line
        }
    }

    /// Create a color picker element
    pub fn color_picker(
        id: impl Into<FocusId>,
        state: &crate::tui::widgets::ColorPickerState,
    ) -> ColorPickerBuilder<Msg> {
        ColorPickerBuilder {
            id: id.into(),
            value: state.color(),
            mode: state.mode(),
            state: state.clone(),
            on_event: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create a text input element
    pub fn text_input(
        id: impl Into<FocusId>,
        value: &str,
        state: &crate::tui::widgets::TextInputState,
    ) -> TextInputBuilder<Msg> {
        TextInputBuilder {
            id: id.into(),
            value: value.to_string(),
            cursor_pos: state.cursor_pos(),
            scroll_offset: state.scroll_offset(),
            placeholder: None,
            max_length: None,
            masked: false,
            on_change: None,
            on_submit: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create a list element from items
    pub fn list<T>(
        id: impl Into<FocusId>,
        items: &[T],
        state: &crate::tui::widgets::ListState,
        theme: &crate::tui::Theme,
    ) -> ListBuilder<Msg>
    where
        T: crate::tui::widgets::ListItem<Msg = Msg>,
    {
        let elements = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = state.selected() == Some(i);
                item.to_element(is_selected, false)
            })
            .collect();

        ListBuilder {
            id: id.into(),
            items: elements,
            selected: state.selected(),
            scroll_offset: state.scroll_offset(),
            on_select: None,
            on_activate: None,
            on_navigate: None,
            on_focus: None,
            on_blur: None,
            on_render: None,
        }
    }

    /// Create a tree element from TreeItem-implementing items
    pub fn tree<T>(
        id: impl Into<FocusId>,
        root_items: &[T],
        state: &mut crate::tui::widgets::TreeState,
        theme: &crate::tui::Theme,
    ) -> TreeBuilder<Msg>
    where
        T: crate::tui::widgets::TreeItem<Msg = Msg>,
    {
        // Force cache invalidation to rebuild visible_order with current items
        state.invalidate_cache();

        // Flatten tree based on expansion state
        let flattened = crate::tui::widgets::tree::flatten_tree(root_items, state);

        // Extract elements and node IDs (parallel arrays) by consuming the vec
        let (elements, node_ids): (Vec<Element<Msg>>, Vec<String>) = flattened
            .into_iter()
            .map(|node| (node.element, node.id))
            .unzip();

        TreeBuilder {
            id: id.into(),
            items: elements,
            node_ids,
            selected: state.selected().map(String::from),
            scroll_offset: state.scroll_offset(),
            on_select: None,
            on_toggle: None,
            on_navigate: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
            on_render: None,
        }
    }

    /// Create a table-style tree element from TableTreeItem-implementing items
    pub fn table_tree<T>(
        id: impl Into<FocusId>,
        root_items: &[T],
        state: &mut crate::tui::widgets::TreeState,
    ) -> TableTreeBuilder<Msg>
    where
        T: crate::tui::widgets::TableTreeItem<Msg = Msg>,
    {
        // Force cache invalidation to rebuild visible_order with current items
        state.invalidate_cache();

        // Flatten tree based on expansion state
        let flattened = crate::tui::widgets::tree::flatten_table_tree(root_items, state);

        // Extract node IDs (parallel array)
        let node_ids: Vec<String> = flattened.iter().map(|node| node.id.clone()).collect();

        // Get column configuration from the trait
        let column_widths = T::column_widths();
        let column_headers = T::column_headers();

        TableTreeBuilder {
            id: id.into(),
            flattened_nodes: flattened,
            node_ids,
            selected: state.selected().map(String::from),
            scroll_offset: state.scroll_offset(),
            column_widths,
            column_headers,
            on_select: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
            on_render: None,
        }
    }

    /// Create a scrollable wrapper around any element
    pub fn scrollable(
        id: impl Into<FocusId>,
        child: Element<Msg>,
        state: &crate::tui::widgets::ScrollableState,
    ) -> ScrollableBuilder<Msg> {
        ScrollableBuilder {
            id: id.into(),
            child: Box::new(child),
            scroll_offset: state.scroll_offset(),
            content_height: None,
            horizontal_scroll_offset: state.horizontal_scroll_offset(),
            content_width: None,
            on_navigate: None,
            on_render: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create a select/dropdown element
    pub fn select(
        id: impl Into<FocusId>,
        options: Vec<String>,
        state: &mut crate::tui::widgets::SelectState,
    ) -> SelectBuilder<Msg> {
        // Update state with current option count
        state.update_option_count(options.len());

        SelectBuilder {
            id: id.into(),
            options,
            selected: state.selected(),
            is_open: state.is_open(),
            highlight: state.highlighted(),
            on_select: None,
            on_toggle: None,
            on_navigate: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create an autocomplete input with fuzzy-matched dropdown
    /// Create a file browser element
    pub fn file_browser(
        id: impl Into<FocusId>,
        state: &crate::tui::widgets::FileBrowserState,
        theme: &crate::tui::Theme,
    ) -> FileBrowserBuilder<Msg> {
        // Convert entries to Elements with proper styling
        let elements = state.entries()
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_selected = state.selected_index() == Some(idx);
                entry.to_element(is_selected)
            })
            .collect();

        FileBrowserBuilder {
            id: id.into(),
            current_path: state.current_path().to_path_buf(),
            entries: elements,
            selected: state.selected_index(),
            scroll_offset: state.list_state().scroll_offset(),
            on_file_selected: None,
            on_directory_changed: None,
            on_directory_entered: None,
            on_navigate: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
            on_render: None,
        }
    }

    pub fn autocomplete(
        id: impl Into<FocusId>,
        all_options: Vec<String>,
        current_input: String,
        state: &mut crate::tui::widgets::AutocompleteState,
    ) -> AutocompleteBuilder<Msg> {
        AutocompleteBuilder {
            id: id.into(),
            all_options,
            current_input,
            placeholder: None,
            is_open: state.is_open(),
            filtered_options: state.filtered_options(),
            highlight: state.highlighted(),
            on_input: None,
            on_select: None,
            on_navigate: None,
            on_event: None,
            on_focus: None,
            on_blur: None,
        }
    }

    pub fn multi_select(
        id: impl Into<FocusId>,
        all_options: Vec<String>,
        state: &mut crate::tui::widgets::MultiSelectState,
    ) -> MultiSelectBuilder<Msg> {
        MultiSelectBuilder {
            id: id.into(),
            all_options,
            selected_items: state.selected_items().to_vec(),
            search_input: state.value().to_string(),
            placeholder: None,
            is_open: state.is_open(),
            filtered_options: state.filtered_options(),
            highlight: state.highlighted(),
            on_event: None,
            on_focus: None,
            on_blur: None,
        }
    }
}

impl<Msg> Default for Element<Msg> {
    fn default() -> Self {
        Element::None
    }
}