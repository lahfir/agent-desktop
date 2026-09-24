use agent_desktop_core::{AdapterError, ErrorCode};

/// Environment variable that selects the background delivery layers.
///
/// Unset or empty selects the path's default ([`BackgroundLayers::pointer_default`]
/// or [`BackgroundLayers::keyboard_default`]), `none` selects the bare
/// `CGEventPostToPid` path, and anything else must be a comma-separated
/// subset of `route`, `skylight`, `auth`, `activate`, `keywindow`, `guard`,
/// and `primer`. It exists so live validation can isolate which technique
/// makes a given app react, and is deliberately kept out of the public CLI.
pub(crate) const LAYERS_ENV: &str = "AGENT_DESKTOP_BG_LAYERS";

const LAYER_NAMES: [&str; 7] = [
    "route",
    "skylight",
    "auth",
    "activate",
    "keywindow",
    "guard",
    "primer",
];

/// Background delivery techniques that can be switched on individually.
///
/// - `route`: window-routing fields (and, for the pointer, the window-local
///   location) so AppKit targets the exact `NSWindow` without a hit test.
/// - `skylight`: post through SkyLight's `SLEventPostToPid` instead of
///   `CGEventPostToPid`.
/// - `auth` (keyboard only): attach an `SLSEventAuthenticationMessage` to
///   each key event, which Chromium targets need on macOS 15+.
/// - `activate`: a target-only window-server focus record so the target's
///   window believes it is focused; nothing is sent to the user's app.
/// - `keywindow` (keyboard only): target-only records that make the window
///   the key window inside its process, where AppKit sends key events.
/// - `guard`: watch the frontmost application and restore the user's app if
///   the target takes it.
/// - `primer` (pointer only): a Chromium primer click outside the window's
///   content before the real click.
///
/// A layer that does not apply to a path is dropped by that path's
/// constructor, so `names()` always lists only layers that ran.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackgroundLayers {
    pub(crate) route: bool,
    pub(crate) skylight: bool,
    pub(crate) auth: bool,
    pub(crate) activate: bool,
    pub(crate) key_window: bool,
    pub(crate) guard: bool,
    pub(crate) primer: bool,
}

impl BackgroundLayers {
    /// Every pointer layer except `primer`, whose extra click is a real input
    /// event the target app sees and is only worth its risk when a Chromium
    /// click is still swallowed with the other layers on.
    pub(crate) fn pointer_default() -> Self {
        Self {
            route: true,
            skylight: true,
            activate: true,
            guard: true,
            ..Self::default()
        }
    }

    /// Every keyboard layer.
    pub(crate) fn keyboard_default() -> Self {
        Self {
            auth: true,
            key_window: true,
            ..Self::pointer_default()
        }
    }

    pub(crate) fn pointer_from_env() -> Result<Self, AdapterError> {
        let layers = Self::parse(env_value().as_deref(), Self::pointer_default())?;
        Ok(Self {
            auth: false,
            key_window: false,
            ..layers
        })
    }

    pub(crate) fn keyboard_from_env() -> Result<Self, AdapterError> {
        let layers = Self::parse(env_value().as_deref(), Self::keyboard_default())?;
        Ok(Self {
            primer: false,
            ..layers
        })
    }

    pub(crate) fn parse(value: Option<&str>, defaults: Self) -> Result<Self, AdapterError> {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(defaults);
        };
        if value.eq_ignore_ascii_case("none") {
            return Ok(Self::default());
        }

        let mut layers = Self::default();
        for token in value
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            match token.to_ascii_lowercase().as_str() {
                "route" => layers.route = true,
                "skylight" => layers.skylight = true,
                "auth" => layers.auth = true,
                "activate" => layers.activate = true,
                "keywindow" => layers.key_window = true,
                "guard" => layers.guard = true,
                "primer" => layers.primer = true,
                _ => return Err(unknown_layer(token)),
            }
        }
        Ok(layers)
    }

    pub(crate) fn names(&self) -> Vec<String> {
        let enabled = [
            self.route,
            self.skylight,
            self.auth,
            self.activate,
            self.key_window,
            self.guard,
            self.primer,
        ];
        LAYER_NAMES
            .iter()
            .zip(enabled)
            .filter(|(_, on)| *on)
            .map(|(name, _)| (*name).to_string())
            .collect()
    }
}

fn env_value() -> Option<String> {
    std::env::var(LAYERS_ENV).ok()
}

fn unknown_layer(token: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!("Unknown {LAYERS_ENV} layer '{token}'"),
    )
    .with_suggestion(format!(
        "Set {LAYERS_ENV} to a comma-separated subset of {}, to 'none', or unset it for the recommended layers",
        LAYER_NAMES.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_or_blank_selects_the_path_default() {
        for value in [None, Some(""), Some("  ")] {
            let pointer = BackgroundLayers::parse(value, BackgroundLayers::pointer_default())
                .expect("pointer default");
            assert_eq!(pointer.names(), ["route", "skylight", "activate", "guard"]);

            let keyboard = BackgroundLayers::parse(value, BackgroundLayers::keyboard_default())
                .expect("keyboard default");
            assert_eq!(
                keyboard.names(),
                [
                    "route",
                    "skylight",
                    "auth",
                    "activate",
                    "keywindow",
                    "guard"
                ]
            );
        }
    }

    #[test]
    fn none_selects_the_bare_post_to_pid_path() {
        let layers = BackgroundLayers::parse(Some("NONE"), BackgroundLayers::keyboard_default())
            .expect("no layers");
        assert_eq!(layers, BackgroundLayers::default());
        assert!(layers.names().is_empty());
    }

    #[test]
    fn a_subset_enables_exactly_the_listed_layers() {
        let layers = BackgroundLayers::parse(
            Some(" route, Primer ,,guard,KeyWindow,AUTH"),
            BackgroundLayers::pointer_default(),
        )
        .expect("subset");
        assert_eq!(
            layers,
            BackgroundLayers {
                route: true,
                auth: true,
                key_window: true,
                guard: true,
                primer: true,
                ..BackgroundLayers::default()
            }
        );
        assert_eq!(
            layers.names(),
            ["route", "auth", "keywindow", "guard", "primer"]
        );
    }

    #[test]
    fn an_unknown_layer_is_rejected_before_delivery() {
        let error = BackgroundLayers::parse(Some("route,warp"), BackgroundLayers::default())
            .expect_err("unknown layer");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
        assert!(error.message.contains("'warp'"));
    }
}
