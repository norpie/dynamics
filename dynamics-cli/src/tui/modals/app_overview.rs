use crate::tui::{Element, Theme, FocusId, AppId, AppLifecycle};
use crate::tui::element::{LayoutConstraint, RowBuilder, ColumnBuilder};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Builder for app overview modal showing all runtime apps and their states
///
/// # Example
/// ```rust
/// let modal = AppOverviewModal::new(apps)
///     .on_close(Msg::CloseModal)
///     .build();
/// ```
pub struct AppOverviewModal<Msg> {
    apps: Vec<(AppId, AppLifecycle)>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> AppOverviewModal<Msg> {
    /// Create a new app overview modal with a list of apps and their states
    pub fn new(apps: Vec<(AppId, AppLifecycle)>) -> Self {
        Self {
            apps,
            on_close: None,
            width: Some(80),
            height: None,
        }
    }

    /// Set the message sent when the modal is closed
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Set modal width (optional, defaults to 60)
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set modal height (optional, auto-sizes by default)
    pub fn height(mut self, height: u16) -> Self {
        self.height = Some(height);
        self
    }

    /// Convert AppLifecycle to a display string with color
    fn lifecycle_to_styled_span(lifecycle: AppLifecycle, theme: &Theme) -> Span<'static> {
        match lifecycle {
            AppLifecycle::NotCreated => Span::styled("Not Created", Style::default().fg(theme.border_tertiary)),
            AppLifecycle::Running => Span::styled("Running", Style::default().fg(theme.accent_success).bold()),
            AppLifecycle::Background => Span::styled("Background", Style::default().fg(theme.accent_tertiary)),
            AppLifecycle::QuittingRequested => Span::styled("Quitting", Style::default().fg(theme.accent_warning)),
            AppLifecycle::Dead => Span::styled("Dead", Style::default().fg(theme.accent_error)),
        }
    }

    /// Convert AppId to a display string
    fn app_id_to_string(app_id: AppId) -> &'static str {
        match app_id {
            AppId::AppLauncher => "App Launcher",
            AppId::LoadingScreen => "Loading Screen",
            AppId::ErrorScreen => "Error Screen",
            AppId::Settings => "Settings",
            AppId::UpdateApp => "Updates",
            AppId::EnvironmentSelector => "Environment Selector",
            AppId::MigrationEnvironment => "Migration Environment",
            AppId::MigrationComparisonSelect => "Migration Comparison Select",
            AppId::EntityComparison => "Entity Comparison",
            AppId::DeadlinesFileSelect => "Deadlines File Select",
            AppId::DeadlinesMapping => "Deadlines Mapping",
            AppId::DeadlinesInspection => "Deadlines Inspection",
            AppId::OperationQueue => "Operation Queue",
            AppId::SelectQuestionnaire => "Select Questionnaire",
            AppId::CopyQuestionnaire => "Copy Questionnaire",
            AppId::PushQuestionnaire => "Push Questionnaire",
            AppId::EntitySync => "Entity Sync",
        }
    }

    /// Build the modal Element
    pub fn build(self) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;

        // Title line
        let title_element = Element::styled_text(Line::from(vec![
            Span::styled("Runtime Apps Overview", Style::default().fg(theme.accent_primary).bold())
        ])).build();

        // Sort apps by lifecycle state (Running, Background, QuittingRequested, Dead, NotCreated)
        let mut apps = self.apps;
        apps.sort_by(|a, b| {
            let order_a = match a.1 {
                AppLifecycle::Running => 0,
                AppLifecycle::Background => 1,
                AppLifecycle::QuittingRequested => 2,
                AppLifecycle::Dead => 3,
                AppLifecycle::NotCreated => 4,
            };
            let order_b = match b.1 {
                AppLifecycle::Running => 0,
                AppLifecycle::Background => 1,
                AppLifecycle::QuittingRequested => 2,
                AppLifecycle::Dead => 3,
                AppLifecycle::NotCreated => 4,
            };
            order_a.cmp(&order_b)
        });

        // Build app list
        let mut app_items: Vec<Element<Msg>> = vec![];

        for (app_id, lifecycle) in apps {
            let app_name = Self::app_id_to_string(app_id);
            let lifecycle_span = Self::lifecycle_to_styled_span(lifecycle, theme);

            let line = Line::from(vec![
                Span::styled(format!("  {:30}", app_name), Style::default().fg(theme.text_primary)),
                Span::raw("  "),
                lifecycle_span,
            ]);

            app_items.push(Element::styled_text(line).build());
        }

        // Extract close message to ensure proper typing
        let close_msg = self.on_close.clone()
            .expect("AppOverviewModal requires on_close callback");

        // Close button
        let close_button = Element::button(
            FocusId::new("app-overview-close"),
            "[ Close ]",
        )
        .on_press(close_msg)
        .style(Style::default().fg(theme.accent_tertiary))
        .build();

        // Build the modal content
        let mut content = ColumnBuilder::new();
        content = content.add(title_element, LayoutConstraint::Length(1));
        content = content.add(Element::text(""), LayoutConstraint::Length(1));

        // Add all app items
        for item in app_items {
            content = content.add(item, LayoutConstraint::Length(1));
        }

        content = content.add(Element::text(""), LayoutConstraint::Length(1));
        content = content.add(close_button, LayoutConstraint::Length(3));

        let content = content.build();

        // Wrap in panel with optional size
        let mut panel = Element::panel(content);

        if let Some(w) = self.width {
            panel = panel.width(w);
        }
        if let Some(h) = self.height {
            panel = panel.height(h);
        }

        panel.build()
    }
}
