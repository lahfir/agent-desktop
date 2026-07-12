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
