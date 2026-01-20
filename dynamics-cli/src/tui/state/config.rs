use super::{FocusMode, Theme, ThemeVariant};
use crate::config::options::Options;
use crate::tui::KeyBinding;
use crate::tui::color::hex_to_color;
use ratatui::style::Color;
use std::collections::HashMap;

/// Runtime configuration for TUI behavior and appearance
///
/// This struct holds all user preferences that affect how the TUI behaves.
/// Currently set statically via Default, but designed to be loaded from
/// config files, environment variables, or CLI arguments in the future.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Visual theme (colors, styles)
    pub theme: Theme,

    /// How keyboard focus is acquired (click, hover, or hybrid)
    pub focus_mode: FocusMode,

    /// Global keybinds mapping action names to key combinations
    pub keybinds: HashMap<String, KeyBinding>,

    /// Tab key debouncing duration in milliseconds
    pub tab_debouncing_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        // Default runtime config with empty keybinds map
        // Keybinds will be looked up from registry via get_keybind()
        Self {
            theme: Theme::new(ThemeVariant::default()),
            focus_mode: FocusMode::default(),
            keybinds: HashMap::new(),
            tab_debouncing_ms: 150,
        }
    }
}

impl RuntimeConfig {
    /// Create a new config with explicit settings
    pub fn new(theme: Theme, focus_mode: FocusMode, keybinds: HashMap<String, KeyBinding>) -> Self {
        let default = Self::default();
        Self {
            theme,
            focus_mode,
            keybinds,
            tab_debouncing_ms: default.tab_debouncing_ms,
        }
    }

    /// Create config with custom theme variant and default focus mode
    pub fn with_theme(variant: ThemeVariant) -> Self {
        let default = Self::default();
        Self {
            theme: Theme::new(variant),
            focus_mode: FocusMode::default(),
            keybinds: default.keybinds,
            tab_debouncing_ms: default.tab_debouncing_ms,
        }
    }

    /// Create config with custom focus mode and default theme
    pub fn with_focus_mode(mode: FocusMode) -> Self {
        let default = Self::default();
        Self {
            theme: Theme::new(ThemeVariant::default()),
            focus_mode: mode,
            keybinds: default.keybinds,
            tab_debouncing_ms: default.tab_debouncing_ms,
        }
    }

    /// Get a keybind by its full key (app.action format).
    ///
    /// Always returns a keybind - either the configured value or the default from the registry.
    /// If no default is registered, returns F24 in release mode and panics in debug mode.
    pub fn get_keybind(&self, key: &str) -> KeyBinding {
        use std::str::FromStr;

        // Try to get from configured keybinds
        if let Some(kb) = self.keybinds.get(key) {
            return *kb;
        }

        // Fall back to default from registry
        let registry = crate::options_registry();
        let option_key = format!("keybind.{}", key);

        if let Some(def) = registry.get(&option_key) {
            // Parse default value
            if let Ok(default_str) = def.default.as_string() {
                if let Ok(kb) = KeyBinding::from_str(&default_str) {
                    return kb;
                }
            }
        }

        // No default registered - panic in debug, return F24 in release
        #[cfg(debug_assertions)]
        panic!(
            "No keybind registered for '{}' - please register it in keybinds.rs",
            key
        );

        #[cfg(not(debug_assertions))]
        {
            log::error!("No keybind registered for '{}' - using F24 fallback", key);
            KeyBinding::from_str("F24").unwrap()
        }
    }

    /// Load runtime config from the options system
    pub async fn load_from_options() -> anyhow::Result<Self> {
        use crate::config::options::registrations::keybinds;
        use std::str::FromStr;

        let config = crate::global_config();

        // Load focus mode from options (defaults to Hover if not found)
        let focus_mode_str = config
            .options
            .get_string("tui.focus_mode")
            .await
            .unwrap_or_else(|_| "hover".to_string());

        let focus_mode = match focus_mode_str.as_str() {
            "click" => FocusMode::Click,
            "hover" => FocusMode::Hover,
            "hover_when_unfocused" => FocusMode::HoverWhenUnfocused,
            _ => FocusMode::default(),
        };

        // Load active theme from options (defaults to mocha if not found)
        let theme_name = config
            .options
            .get_string("theme.active")
            .await
            .unwrap_or_else(|_| "mocha".to_string());

        // Load theme colors from options database
        let theme = load_theme_from_options(&config.options, &theme_name)
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to load theme '{}': {}. Falling back to mocha.",
                    theme_name,
                    e
                );
                Theme::mocha()
            });

        // Load tab debouncing from options (defaults to 150ms if not found)
        let tab_debouncing_ms = config
            .options
            .get_uint("keys.tab.debouncing")
            .await
            .unwrap_or_else(|_| 150);

        // Load keybinds from options database (now app-scoped)
        let mut keybinds = HashMap::new();
        let apps = keybinds::list_apps(&config.options.registry());

        for app in apps {
            let actions = keybinds::list_actions_for_app(&config.options.registry(), &app);

            for action in actions {
                let full_key = format!("{}.{}", app, action);
                let option_key = format!("keybind.{}.{}", app, action);

                let keybind_str =
                    config
                        .options
                        .get_string(&option_key)
                        .await
                        .unwrap_or_else(|_| {
                            // Fall back to default keybind if not found
                            let default_config = Self::default();
                            default_config
                                .keybinds
                                .get(&full_key)
                                .map(|kb| kb.to_string())
                                .unwrap_or_else(|| "F1".to_string())
                        });

                match KeyBinding::from_str(&keybind_str) {
                    Ok(keybind) => {
                        keybinds.insert(full_key.clone(), keybind);
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse keybind '{}' for action '{}': {}. Using default.",
                            keybind_str,
                            full_key,
                            e
                        );
                        // Use default keybind for this action
                        let default_config = Self::default();
                        if let Some(default_kb) = default_config.keybinds.get(&full_key) {
                            keybinds.insert(full_key.clone(), *default_kb);
                        }
                    }
                }
            }
        }

        Ok(Self {
            theme,
            focus_mode,
            keybinds,
            tab_debouncing_ms,
        })
    }
}

/// Load a theme by name from the options database
///
/// Returns an error if the theme doesn't exist or if any required color is missing
async fn load_theme_from_options(options: &Options, name: &str) -> anyhow::Result<Theme> {
    let accent_primary = load_color(options, name, "accent_primary").await?;
    let accent_secondary = load_color(options, name, "accent_secondary").await?;
    let accent_tertiary = load_color(options, name, "accent_tertiary").await?;
    let accent_error = load_color(options, name, "accent_error").await?;
    let accent_warning = load_color(options, name, "accent_warning").await?;
    let accent_success = load_color(options, name, "accent_success").await?;
    let accent_info = load_color(options, name, "accent_info").await?;
    let accent_muted = load_color(options, name, "accent_muted").await?;

    let text_primary = load_color(options, name, "text_primary").await?;
    let text_secondary = load_color(options, name, "text_secondary").await?;
    let text_tertiary = load_color(options, name, "text_tertiary").await?;

    let border_primary = load_color(options, name, "border_primary").await?;
    let border_secondary = load_color(options, name, "border_secondary").await?;
    let border_tertiary = load_color(options, name, "border_tertiary").await?;

    let bg_base = load_color(options, name, "bg_base").await?;
    let bg_surface = load_color(options, name, "bg_surface").await?;
    let bg_elevated = load_color(options, name, "bg_elevated").await?;

    let palette_1 = load_color(options, name, "palette_1").await?;
    let palette_2 = load_color(options, name, "palette_2").await?;
    let palette_3 = load_color(options, name, "palette_3").await?;
    let palette_4 = load_color(options, name, "palette_4").await?;

    Ok(Theme {
        accent_primary,
        accent_secondary,
        accent_tertiary,
        accent_error,
        accent_warning,
        accent_success,
        accent_info,
        accent_muted,
        text_primary,
        text_secondary,
        text_tertiary,
        border_primary,
        border_secondary,
        border_tertiary,
        bg_base,
        bg_surface,
        bg_elevated,
        palette_1,
        palette_2,
        palette_3,
        palette_4,
    })
}

/// Load all themes by their names from the options database
///
/// Returns a HashMap mapping theme names to Theme structs.
/// Themes that fail to load are logged and skipped.
pub async fn load_all_themes(
    options: &Options,
    names: Vec<String>,
) -> std::collections::HashMap<String, Theme> {
    let mut themes = std::collections::HashMap::new();

    for name in names {
        match load_theme_from_options(options, &name).await {
            Ok(theme) => {
                themes.insert(name, theme);
            }
            Err(e) => {
                log::warn!("Failed to load theme '{}': {}. Skipping.", name, e);
            }
        }
    }

    themes
}

/// Load a single color from options by theme name and color name
async fn load_color(options: &Options, theme: &str, color: &str) -> anyhow::Result<Color> {
    let key = format!("theme.{}.{}", theme, color);
    let hex = options.get_string(&key).await?;
    hex_to_color(&hex)
}
