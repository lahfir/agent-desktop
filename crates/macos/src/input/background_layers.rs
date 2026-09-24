use agent_desktop_core::{AdapterError, ErrorCode};

/// Environment variable that selects the background pointer delivery layers.
///
/// Unset or empty selects [`BackgroundLayers::recommended`], `none` selects
/// the bare `CGEventPostToPid` path, and anything else must be a
/// comma-separated subset of `route`, `skylight`, `activate`, `guard`, and
/// `primer`. It exists so live validation can isolate which technique makes a
/// given app react, and is deliberately kept out of the public CLI.
pub(crate) const LAYERS_ENV: &str = "AGENT_DESKTOP_BG_LAYERS";

const LAYER_NAMES: [&str; 5] = ["route", "skylight", "activate", "guard", "primer"];

/// Background pointer techniques that can be switched on individually.
///
/// - `route`: window-routing fields beyond 91/92 plus the window-local
///   location, so AppKit targets the exact `NSWindow` without a hit test.
/// - `skylight`: post through SkyLight's `SLEventPostToPid` instead of
///   `CGEventPostToPid`.
/// - `activate`: a target-only window-server focus record so the target's
///   window believes it is focused; nothing is sent to the user's app.
/// - `guard`: watch the frontmost application and restore the user's app if
///   the target takes it.
/// - `primer`: a Chromium primer click outside the window's content before
///   the real click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackgroundLayers {
    pub(crate) route: bool,
    pub(crate) skylight: bool,
    pub(crate) activate: bool,
    pub(crate) guard: bool,
    pub(crate) primer: bool,
}

impl BackgroundLayers {
    /// Every layer except `primer`, whose extra click is a real input event
    /// the target app sees and is only worth its risk when a Chromium click is
    /// still swallowed with the other layers on.
    pub(crate) fn recommended() -> Self {
        Self {
            route: true,
            skylight: true,
            activate: true,
            guard: true,
            primer: false,
        }
    }

    pub(crate) fn from_env() -> Result<Self, AdapterError> {
        Self::parse(std::env::var(LAYERS_ENV).ok().as_deref())
    }

    pub(crate) fn parse(value: Option<&str>) -> Result<Self, AdapterError> {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(Self::recommended());
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
                "activate" => layers.activate = true,
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
            self.activate,
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
    fn unset_or_blank_selects_every_layer_but_the_primer() {
        for value in [None, Some(""), Some("  ")] {
            let layers = BackgroundLayers::parse(value).expect("default layers");
            assert_eq!(layers, BackgroundLayers::recommended());
            assert_eq!(layers.names(), ["route", "skylight", "activate", "guard"]);
        }
    }

    #[test]
    fn none_selects_the_bare_post_to_pid_path() {
        let layers = BackgroundLayers::parse(Some("NONE")).expect("no layers");
        assert_eq!(layers, BackgroundLayers::default());
        assert!(layers.names().is_empty());
    }

    #[test]
    fn a_subset_enables_exactly_the_listed_layers() {
        let layers = BackgroundLayers::parse(Some(" route, Primer ,,guard")).expect("subset");
        assert_eq!(
            layers,
            BackgroundLayers {
                route: true,
                guard: true,
                primer: true,
                ..BackgroundLayers::default()
            }
        );
        assert_eq!(layers.names(), ["route", "guard", "primer"]);
    }

    #[test]
    fn an_unknown_layer_is_rejected_before_delivery() {
        let error = BackgroundLayers::parse(Some("route,warp")).expect_err("unknown layer");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
        assert!(error.message.contains("'warp'"));
    }
}
