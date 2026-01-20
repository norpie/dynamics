/// UI layout and pattern macros for ergonomic view construction
///
/// This module provides declarative macros to reduce boilerplate in view functions.
/// See layout.md for design rationale and usage examples.

/// Create a spacer element for vertical/horizontal gaps
///
/// # Examples
/// ```
/// spacer!()     // 1 line gap (Element::text(""))
/// spacer!(3)    // 3 line gap (column of empty text)
/// ```
#[macro_export]
macro_rules! spacer {
    () => {
        $crate::tui::Element::text("")
    };
    ($height:expr) => {{
        let items: Vec<_> = (0..$height)
            .map(|_| {
                (
                    $crate::tui::LayoutConstraint::Length(1),
                    $crate::tui::Element::text(""),
                )
            })
            .collect();
        $crate::tui::element::ColumnBuilder::from_items(items)
            .spacing(0)
            .build()
    }};
}

/// Create a vertical column layout
///
/// # Examples
/// ```
/// // Simple: all children get Fill(1) constraint
/// col![
///     Element::text("Header"),
///     Element::text("Body"),
///     Element::text("Footer"),
/// ]
///
/// // With explicit constraints using => syntax
/// col![
///     Element::text("Header") => Length(1),
///     list => Fill(1),
///     Element::text("Footer") => Length(1),
/// ]
/// ```
#[macro_export]
macro_rules! col {
    // Without constraints - use Fill(1) default
    [ $($child:expr),* $(,)? ] => {{
        let mut builder = $crate::tui::element::ColumnBuilder::new();
        $(
            builder = builder.add($child, $crate::tui::LayoutConstraint::Fill(1));
        )*
        builder.build()
    }};

    // With explicit constraints using => syntax
    [ $($child:expr => $constraint:expr),* $(,)? ] => {{
        let mut builder = $crate::tui::element::ColumnBuilder::new();
        $(
            builder = builder.add($child, $constraint);
        )*
        builder.build()
    }};
}

/// Create a horizontal row layout
///
/// # Examples
/// ```
/// // Simple: all children get Fill(1) constraint
/// row![
///     Element::button("cancel", "Cancel"),
///     Element::button("confirm", "Confirm"),
/// ]
///
/// // With explicit constraints using => syntax
/// row![
///     sidebar => Length(20),
///     content => Fill(1),
///     details => Length(30),
/// ]
/// ```
#[macro_export]
macro_rules! row {
    // Without constraints - use Fill(1) default
    [ $($child:expr),* $(,)? ] => {{
        let mut builder = $crate::tui::element::RowBuilder::new();
        $(
            builder = builder.add($child, $crate::tui::LayoutConstraint::Fill(1));
        )*
        builder.build()
    }};

    // With explicit constraints using => syntax
    [ $($child:expr => $constraint:expr),* $(,)? ] => {{
        let mut builder = $crate::tui::element::RowBuilder::new();
        $(
            builder = builder.add($child, $constraint);
        )*
        builder.build()
    }};
}

/// Import all layout constraint types for shorter syntax
///
/// # Example
/// ```
/// use_constraints!();
/// col![
///     thing @ Length(3),  // no need for LayoutConstraint::Length
///     thing @ Fill(1),
/// ]
/// ```
#[macro_export]
macro_rules! use_constraints {
    () => {
        use $crate::tui::LayoutConstraint::{Fill, Length, Min};
    };
}

/// Create a button row with consistent spacing
///
/// # Example
/// ```
/// button_row![
///     ("cancel", "Cancel", Msg::Cancel),
///     ("confirm", "Confirm", Msg::Confirm),
/// ]
/// ```
#[macro_export]
macro_rules! button_row {
    [ $(($id:expr, $label:expr, $msg:expr)),* $(,)? ] => {{
        let mut builder = $crate::tui::element::RowBuilder::new();
        let mut idx = 0;
        $(
            if idx > 0 {
                builder = builder.add(
                    $crate::tui::Element::text("  "),
                    $crate::tui::LayoutConstraint::Length(2)
                );
            }
            builder = builder.add(
                $crate::tui::Element::button($id, $label)
                    .on_press($msg)
                    .build(),
                $crate::tui::LayoutConstraint::Fill(1)
            );
            idx += 1;
        )*
        builder.spacing(0).build()
    }};
}

/// Create a modal overlay (dimmed background with centered content)
///
/// # Examples
/// ```
/// modal!(main_ui, modal_content)
/// modal!(main_ui, modal_content, Alignment::TopRight)
/// ```
#[macro_export]
macro_rules! modal {
    ($bg_base:expr, $overlay:expr) => {
        $crate::tui::Element::stack(vec![
            $crate::tui::Layer::new($base),
            $crate::tui::Layer::new($overlay).center().dim(true),
        ])
    };
    ($bg_base:expr, $overlay:expr, $align:expr) => {
        $crate::tui::Element::stack(vec![
            $crate::tui::Layer::new($base),
            $crate::tui::Layer::new($overlay).align($align).dim(true),
        ])
    };
}

/// Display validation error with warning icon
///
/// # Example
/// ```
/// error_display!(state.form.validation_error, theme)
/// ```
#[macro_export]
macro_rules! error_display {
    ($error_opt:expr, $theme:expr) => {
        if let Some(ref err) = $error_opt {
            $crate::col![
                $crate::tui::Element::styled_text(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(
                        format!("⚠ {}", err),
                        ratatui::style::Style::default().fg($theme.accent_error)
                    )
                ]))
                .build() => $crate::tui::LayoutConstraint::Length(1),
                $crate::spacer!() => $crate::tui::LayoutConstraint::Length(1),
            ]
        } else {
            $crate::tui::Element::text("")
        }
    };
}

/// Create a labeled text input field in a panel
///
/// # Example
/// ```
/// labeled_input!(
///     "Name",
///     "create-name-input",
///     &state.create_form.name,
///     &state.create_form.name_input_state,
///     Msg::CreateFormNameChanged
/// )
/// ```
#[macro_export]
macro_rules! labeled_input {
    ($title:expr, $id:expr, $value:expr, $state:expr, $on_change:expr) => {
        $crate::tui::Element::panel(
            $crate::tui::Element::text_input($id, $value, $state)
                .on_change($on_change)
                .build(),
        )
        .title($title)
        .build()
    };
    // With placeholder
    ($title:expr, $id:expr, $value:expr, $state:expr, $on_change:expr, $placeholder:expr) => {
        $crate::tui::Element::panel(
            $crate::tui::Element::text_input($id, $value, $state)
                .placeholder($placeholder)
                .on_change($on_change)
                .build(),
        )
        .title($title)
        .build()
    };
}

/// Load data into a Resource field with automatic Loading state
///
/// Sets the field to Loading, executes async future, and returns a Command
/// that wraps the result in Resource::from_result().
///
/// # Examples
/// ```
/// // In update handler - requires a Msg variant to receive the result
/// Msg::LoadData => {
///     state.data = Resource::Loading;
///     Command::perform(fetch_data(), Msg::DataLoaded)
/// }
/// Msg::DataLoaded(result) => {
///     state.data = Resource::from_result(result);
///     Command::None
/// }
/// ```
///
/// Note: This is a documentation macro pattern. The actual implementation
/// is done manually in update handlers since we can't capture state mutably
/// in the macro closure.

/// Declarative subscriptions macro
///
/// Provides a more readable way to define subscriptions with conditional logic
/// and key aliases.
///
/// # Examples
/// ```
/// subscriptions! {
///     // Conditional timer
///     timer!(1ms, when: !state.initialized, Msg::Initialize);
///
///     // Conditional keyboard subscriptions
///     when(!state.show_modal) {
///         key!('n' | 'N', "Create new", Msg::Create);
///         key!('d' | 'D', "Delete", Msg::Delete);
///     }
///
///     // Event subscription
///     event!("migration:selected", |data| {
///         serde_json::from_value::<Metadata>(data).ok().map(Msg::Init)
///     });
/// }
/// ```
#[macro_export]
macro_rules! subscriptions {
    (
        $($item:tt)*
    ) => {{
        let mut subs = Vec::new();
        $crate::subscriptions_impl!(subs; $($item)*);
        subs
    }};
}

/// Internal implementation macro for subscriptions
#[macro_export]
macro_rules! subscriptions_impl {
    // Base case: empty
    ($subs:ident;) => {};

    // timer! macro
    ($subs:ident; timer!($dur:expr, when: $cond:expr, $msg:expr); $($rest:tt)*) => {
        if $cond {
            $subs.push($crate::tui::Subscription::timer($dur, $msg));
        }
        $crate::subscriptions_impl!($subs; $($rest)*);
    };

    // key! macro with single key
    ($subs:ident; key!($key:expr, $desc:expr, $msg:expr); $($rest:tt)*) => {
        $subs.push($crate::tui::Subscription::keyboard(
            crossterm::event::KeyCode::Char($key),
            $desc,
            $msg
        ));
        $crate::subscriptions_impl!($subs; $($rest)*);
    };

    // key! macro with multiple keys (aliases) - using tt to allow |
    ($subs:ident; key!($key1:tt | $key2:tt, $desc:expr, $msg:expr); $($rest:tt)*) => {
        $subs.push($crate::tui::Subscription::keyboard(
            crossterm::event::KeyCode::Char($key1),
            $desc,
            $msg.clone()
        ));
        $subs.push($crate::tui::Subscription::keyboard(
            crossterm::event::KeyCode::Char($key2),
            $desc,
            $msg
        ));
        $crate::subscriptions_impl!($subs; $($rest)*);
    };

    // when block
    ($subs:ident; when($cond:expr) { $($inner:tt)* } $($rest:tt)*) => {
        if $cond {
            $crate::subscriptions_impl!($subs; $($inner)*);
        }
        $crate::subscriptions_impl!($subs; $($rest)*);
    };

    // event! macro
    ($subs:ident; event!($topic:expr, $handler:expr); $($rest:tt)*) => {
        $subs.push($crate::tui::Subscription::subscribe($topic, $handler));
        $crate::subscriptions_impl!($subs; $($rest)*);
    };
}

/// Form builder DSL for creating forms with automatic field wrapping and layout
///
/// Reduces form UI code by ~60% by auto-generating:
/// - Panel wrappers with titles
/// - Layout constraints
/// - Event handler wiring
///
/// # Examples
/// ```
/// form_layout! {
///     theme: theme,
///     fields: [
///         text("Name", "name-id", field.value().to_string(), &mut field.state, Msg::NameEvent) => Length(3),
///         text("Desc", "desc-id", value, &mut state, Msg::DescEvent, placeholder: "Optional") => Length(3),
///         spacer => Length(1),
///         select("Source", "source-id", &mut state, Msg::SourceEvent, options.clone()) => Length(10),
///         autocomplete("Entity", "entity-id", value, &mut state, Msg::EntityEvent, options.clone()) => Length(10),
///         spacer => Length(1),
///         error(state.form.error) => Length(2),
///     ],
///     buttons: [
///         ("cancel-btn", "Cancel", Msg::Cancel),
///         ("submit-btn", "Submit", Msg::Submit),
///     ]
/// }
/// ```
#[macro_export]
macro_rules! form_layout {
    (
        theme: $theme:expr,
        fields: [ $($field:tt)* ],
        buttons: [ $($button:tt)* ]
    ) => {{
        let mut items = Vec::new();
        $crate::form_fields_impl!(items, $theme; $($field)*);

        // Add button row
        let button_row = $crate::button_row![ $($button)* ];
        items.push(($crate::tui::LayoutConstraint::Length(3), button_row));

        $crate::tui::element::ColumnBuilder::from_items(items).build()
    }};
}

/// Internal implementation macro for form fields
#[macro_export]
macro_rules! form_fields_impl {
    // Base case: empty
    ($items:ident, $theme:expr;) => {};

    // text field without options
    ($items:ident, $theme:expr; text($title:expr, $id:expr, $value:expr, $state:expr, $msg:expr) => $constraint:expr, $($rest:tt)*) => {
        {
            let input = $crate::tui::Element::text_input(
                $crate::tui::FocusId::new($id),
                $value,
                $state
            )
            .on_event($msg)
            .build();

            let panel = $crate::tui::Element::panel(input)
                .title($title)
                .build();

            $items.push(($constraint, panel));
        }
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };

    // text field with placeholder
    ($items:ident, $theme:expr; text($title:expr, $id:expr, $value:expr, $state:expr, $msg:expr, placeholder: $placeholder:expr) => $constraint:expr, $($rest:tt)*) => {
        {
            let val_str = $value;
            let input = $crate::tui::Element::text_input(
                $crate::tui::FocusId::new($id),
                val_str.as_ref(),
                $state
            )
            .placeholder($placeholder)
            .on_event($msg)
            .build();

            let panel = $crate::tui::Element::panel(input)
                .title($title)
                .build();

            $items.push(($constraint, panel));
        }
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };

    // select field
    ($items:ident, $theme:expr; select($title:expr, $id:expr, $state:expr, $msg:expr, $options:expr) => $constraint:expr, $($rest:tt)*) => {
        {
            let select = $crate::tui::Element::select(
                $crate::tui::FocusId::new($id),
                $options,
                $state
            )
            .on_event($msg)
            .build();

            let panel = $crate::tui::Element::panel(select)
                .title($title)
                .build();

            $items.push(($constraint, panel));
        }
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };

    // autocomplete field
    ($items:ident, $theme:expr; autocomplete($title:expr, $id:expr, $value:expr, $state:expr, $msg:expr, $options:expr) => $constraint:expr, $($rest:tt)*) => {
        {
            let autocomplete = $crate::tui::Element::autocomplete(
                $crate::tui::FocusId::new($id),
                $options,
                $value,
                $state
            )
            .on_event($msg)
            .build();

            let panel = $crate::tui::Element::panel(autocomplete)
                .title($title)
                .build();

            $items.push(($constraint, panel));
        }
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };

    // spacer
    ($items:ident, $theme:expr; spacer => $constraint:expr, $($rest:tt)*) => {
        $items.push(($constraint, $crate::tui::Element::text("")));
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };

    // error display
    ($items:ident, $theme:expr; error($error:expr) => $constraint:expr, $($rest:tt)*) => {
        $items.push(($constraint, $crate::error_display!($error, $theme)));
        $crate::form_fields_impl!($items, $theme; $($rest)*);
    };
}
