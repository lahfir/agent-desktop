#[cfg(target_os = "macos")]
use agent_desktop_core::{
    error::{AdapterError, ErrorCode},
    interaction_policy::InteractionPolicy,
};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn execute_type(
    el: &AXElement,
    text: &str,
    policy: InteractionPolicy,
) -> Result<(), AdapterError> {
    match type_via_ax_value(el, text) {
        Ok(()) => return Ok(()),
        Err(err) if !policy.allow_focus_steal => return Err(err),
        Err(_) => {}
    }

    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let _ = crate::system::app_ops::ensure_app_focused(pid);
    }
    crate::actions::ax_helpers::ax_focus_or_err(el)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    if !text.is_ascii() {
        return type_via_clipboard_paste(el, text);
    }
    crate::input::keyboard::synthesize_text(text)
}

/// Restores the user's clipboard on every scope exit — success, error, or
/// panic — so a failure or early return mid-paste cannot leave the pasted text
/// behind. (A SIGKILL between set and restore is unpreventable in any process.)
#[cfg(target_os = "macos")]
struct ClipboardRestore {
    previous: crate::input::clipboard::ClipboardSnapshot,
}

#[cfg(target_os = "macos")]
impl Drop for ClipboardRestore {
    fn drop(&mut self) {
        let _ = self.previous.restore();
    }
}

#[cfg(target_os = "macos")]
fn type_via_clipboard_paste(el: &AXElement, text: &str) -> Result<(), AdapterError> {
    let before = readable_value(el);
    let previous = crate::input::clipboard::ClipboardSnapshot::capture()?;
    let _restore = ClipboardRestore { previous };
    crate::input::clipboard::set(text)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    let combo = agent_desktop_core::action::KeyCombo {
        key: "v".into(),
        modifiers: vec![agent_desktop_core::action::Modifier::Cmd],
    };
    crate::input::keyboard::synthesize_key(&combo)?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    verify_paste_effect(before.as_deref(), readable_value(el).as_deref())
}

#[cfg(target_os = "macos")]
fn type_via_ax_value(el: &AXElement, text: &str) -> Result<(), AdapterError> {
    if !is_text_target(el) || !crate::actions::ax_helpers::is_attr_settable(el, "AXValue") {
        return Err(AdapterError::policy_denied(
            "Headless typing requires a settable text value; use set-value or an explicit focus command",
        ));
    }

    let current = if is_secure_text_field(el) {
        String::new()
    } else {
        crate::tree::copy_value_typed(el).unwrap_or_default()
    };
    let next = typed_value(&current, text);
    crate::actions::ax_helpers::ax_set_value(el, &next)?;
    if is_secure_text_field(el) {
        return Ok(());
    }
    let after = crate::tree::copy_value_typed(el).unwrap_or_default();
    if after == next {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "AX value write reported success but the element value did not change",
    )
    .with_suggestion(
        "Use explicit keyboard input for web-backed fields that ignore AXValue writes",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn execute_type(
    _el: &crate::tree::AXElement,
    _text: &str,
    _policy: agent_desktop_core::interaction_policy::InteractionPolicy,
) -> Result<(), agent_desktop_core::error::AdapterError> {
    Err(agent_desktop_core::error::AdapterError::new(
        agent_desktop_core::error::ErrorCode::PlatformNotSupported,
        "type_text is not supported on this platform",
    ))
}

#[cfg(target_os = "macos")]
fn is_text_target(el: &AXElement) -> bool {
    matches!(
        crate::actions::ax_helpers::element_role(el).as_deref(),
        Some("textfield" | "combobox")
    )
}

#[cfg(target_os = "macos")]
fn is_secure_text_field(el: &AXElement) -> bool {
    crate::tree::copy_string_attr(el, "AXRole").as_deref() == Some("AXSecureTextField")
}

#[cfg(target_os = "macos")]
fn readable_value(el: &AXElement) -> Option<String> {
    if is_secure_text_field(el) {
        None
    } else {
        crate::tree::copy_value_typed(el)
    }
}

#[cfg(target_os = "macos")]
fn verify_paste_effect(before: Option<&str>, after: Option<&str>) -> Result<(), AdapterError> {
    if before.is_none() || after.is_none() || before != after {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "Clipboard paste completed but the target value did not change",
    )
    .with_suggestion(
        "Use set-value for fields that expose AXValue, or retry with physical keyboard input.",
    ))
}

#[cfg(target_os = "macos")]
fn typed_value(current: &str, text: &str) -> String {
    let mut value = String::with_capacity(current.len() + text.len());
    value.push_str(current);
    value.push_str(text);
    value
}

#[cfg(test)]
mod tests {
    #[test]
    fn typed_value_appends_without_losing_existing_text() {
        assert_eq!(super::typed_value("abc", "123"), "abc123");
    }

    #[test]
    fn paste_verification_rejects_readable_no_change() {
        let err = super::verify_paste_effect(Some("before"), Some("before")).unwrap_err();
        assert_eq!(err.code, agent_desktop_core::error::ErrorCode::ActionFailed);
    }

    #[test]
    fn paste_verification_accepts_unreadable_or_changed_values() {
        assert!(super::verify_paste_effect(None, Some("after")).is_ok());
        assert!(super::verify_paste_effect(Some("before"), None).is_ok());
        assert!(super::verify_paste_effect(Some("before"), Some("after")).is_ok());
    }
}
