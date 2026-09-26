pub(crate) fn bounded_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let prefix = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_none() {
        return prefix;
    }
    format!("{prefix}… <truncated; {} bytes total>", text.len())
}

pub(crate) fn token_label(token: &str) -> String {
    if token.len() <= 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        format!("'{token}'")
    } else {
        format!("token of {} bytes", token.len())
    }
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;

/// Collapses a clap parse error into one line that still names the offending
/// argument.
///
/// Clap puts its headline on the first line and the argument names on the
/// lines below it, then a blank line, then usage and help boilerplate. Taking
/// only the first line yields "the following required arguments were not
/// provided:" with nothing after the colon, which tells a caller nothing about
/// what to fix. This keeps everything up to the blank line and drops the
/// boilerplate a machine consumer cannot act on.
pub(crate) fn clap_error_summary(message: &str) -> String {
    let summary = message
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if summary.is_empty() {
        return "parse error".to_owned();
    }
    summary
}

#[cfg(test)]
#[path = "diagnostic_clap_tests.rs"]
mod clap_tests;
