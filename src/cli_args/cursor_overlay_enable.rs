use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct CursorOverlayEnableArgs {
    #[arg(
        long,
        help = "Show caller-authored intent text beside the cursor overlay"
    )]
    pub label: Option<String>,
    #[arg(
        long,
        value_parser = parse_word_limit,
        help = "Limit the cursor overlay label to 1-12 words (default 6)"
    )]
    pub max_words: Option<usize>,
    #[arg(long, help = "Cursor body colour as a hex value (default #FFFFFF)")]
    pub fill: Option<String>,
    #[arg(long, help = "Cursor outline colour as a hex value (default #111318)")]
    pub rim: Option<String>,
    #[arg(
        long,
        help = "Ripple and element highlight colour as a hex value (default #4299FF)"
    )]
    pub accent: Option<String>,
    #[arg(long, help = "Cursor size multiplier from 0.5 to 4.0 (default 1.0)")]
    pub size: Option<f64>,
    #[arg(long, help = "Do not play the ripple when the agent clicks")]
    pub no_ripple: bool,
    #[arg(long, help = "Do not outline the element when the agent clicks")]
    pub no_highlight: bool,
}

impl CursorOverlayEnableArgs {
    pub(crate) fn to_core(
        &self,
    ) -> Result<agent_desktop_core::CursorOverlayConfig, agent_desktop_core::AppError> {
        use agent_desktop_core::{CursorOverlayConfig, CursorOverlayStyle};

        let mut style = CursorOverlayStyle::default();
        if let Some(fill) = self.fill.clone() {
            style.set_fill(fill);
        }
        if let Some(rim) = self.rim.clone() {
            style.set_rim(rim);
        }
        if let Some(accent) = self.accent.clone() {
            style.set_accent(accent);
        }
        if let Some(size) = self.size {
            style.set_size(size);
        }
        style.set_effects(!self.no_ripple, !self.no_highlight);
        CursorOverlayConfig::enabled(self.label.clone(), self.max_words.unwrap_or(6))
            .and_then(|config| config.with_style(style))
            .map_err(Into::into)
    }
}

fn parse_word_limit(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "word limit must be an integer from 1 to 12".to_string())?;
    if (1..=12).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("word limit must be from 1 to 12".to_string())
    }
}
