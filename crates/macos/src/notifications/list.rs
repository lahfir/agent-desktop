use agent_desktop_core::{
    error::AdapterError,
    notification::{NotificationFilter, NotificationInfo},
};

use super::NcSession;

pub fn list_notifications(
    filter: &NotificationFilter,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    let session = NcSession::open()?;
    let result = list_from_nc(filter);
    session.close()?;
    result
}

#[cfg(target_os = "macos")]
fn list_from_nc(filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError> {
    let entries = list_entries(filter)?;
    Ok(entries.into_iter().map(|e| e.info).collect())
}

#[cfg(target_os = "macos")]
pub(super) struct NotificationEntry {
    pub info: NotificationInfo,
    pub element: crate::tree::AXElement,
}

#[cfg(target_os = "macos")]
pub(super) fn list_entries(
    filter: &NotificationFilter,
) -> Result<Vec<NotificationEntry>, AdapterError> {
    use crate::tree::{copy_ax_array, element_for_pid};
    use accessibility_sys::kAXChildrenAttribute;

    let pid = super::nc_session::nc_pid()
        .ok_or_else(|| AdapterError::internal("Notification Center process not found"))?;

    let app = element_for_pid(pid);
    let windows = copy_ax_array(&app, "AXWindows").unwrap_or_default();
    if windows.is_empty() {
        return Ok(vec![]);
    }

    let app_filter = filter.app.as_deref().map(|s| s.to_lowercase());
    let text_filter = filter.text.as_deref().map(|s| s.to_lowercase());
    let limit = filter.limit.unwrap_or(usize::MAX);

    let mut entries = Vec::new();
    let mut index: usize = 1;

    for window in &windows {
        let top_children = copy_ax_array(window, kAXChildrenAttribute).unwrap_or_default();
        collect_notifications(
            &top_children,
            &app_filter,
            &text_filter,
            limit,
            &mut index,
            &mut entries,
            0,
        );
        if entries.len() >= limit {
            break;
        }
    }

    Ok(entries)
}

#[cfg(target_os = "macos")]
fn collect_notifications(
    elements: &[crate::tree::AXElement],
    app_filter: &Option<String>,
    text_filter: &Option<String>,
    limit: usize,
    index: &mut usize,
    out: &mut Vec<NotificationEntry>,
    depth: u8,
) {
    use crate::tree::{copy_ax_array, copy_string_attr};
    use accessibility_sys::{kAXChildrenAttribute, kAXRoleAttribute};

    if depth > 10 || out.len() >= limit {
        return;
    }

    for el in elements {
        if out.len() >= limit {
            return;
        }

        let role = copy_string_attr(el, kAXRoleAttribute);
        let children = copy_ax_array(el, kAXChildrenAttribute).unwrap_or_default();

        if is_notification_group(role.as_deref(), &children) {
            if let Some(info) = extract_notification(el, &children, *index) {
                if matches_filters(&info, app_filter, text_filter) {
                    out.push(NotificationEntry {
                        info,
                        element: el.clone(),
                    });
                }
                *index += 1;
                continue;
            }
        }

        collect_notifications(
            &children,
            app_filter,
            text_filter,
            limit,
            index,
            out,
            depth + 1,
        );
    }
}

#[cfg(target_os = "macos")]
fn is_notification_group(role: Option<&str>, children: &[crate::tree::AXElement]) -> bool {
    use crate::tree::copy_string_attr;
    use accessibility_sys::kAXRoleAttribute;

    if role != Some("AXGroup") {
        return false;
    }
    let has_static_text = children
        .iter()
        .any(|c| copy_string_attr(c, kAXRoleAttribute).as_deref() == Some("AXStaticText"));
    let has_button = children
        .iter()
        .any(|c| copy_string_attr(c, kAXRoleAttribute).as_deref() == Some("AXButton"));
    has_static_text || has_button
}

#[cfg(target_os = "macos")]
fn extract_notification(
    _group: &crate::tree::AXElement,
    children: &[crate::tree::AXElement],
    index: usize,
) -> Option<NotificationInfo> {
    use crate::tree::copy_string_attr;
    use accessibility_sys::{kAXRoleAttribute, kAXValueAttribute};

    let mut texts: Vec<String> = Vec::new();
    let mut actions: Vec<String> = Vec::new();

    for child in children {
        let role = copy_string_attr(child, kAXRoleAttribute);
        match role.as_deref() {
            Some("AXStaticText") => {
                if let Some(val) = copy_string_attr(child, kAXValueAttribute) {
                    if !val.is_empty() {
                        texts.push(val);
                    }
                }
            }
            Some("AXButton") => {
                let name = copy_string_attr(child, "AXTitle")
                    .or_else(|| copy_string_attr(child, "AXDescription"));
                if let Some(n) = name {
                    if !n.is_empty() && n != "Close" && n != "clear" {
                        actions.push(n);
                    }
                }
            }
            _ => {}
        }
    }

    if texts.is_empty() {
        return None;
    }

    let app_name = if texts.len() >= 2 {
        texts[0].clone()
    } else {
        String::from("Unknown")
    };

    let title = if texts.len() >= 2 {
        texts[1].clone()
    } else {
        texts[0].clone()
    };

    let body = if texts.len() >= 3 {
        Some(texts[2..].join(" "))
    } else {
        None
    };

    Some(NotificationInfo {
        index,
        app_name,
        title,
        body,
        timestamp: None,
        actions,
    })
}

fn matches_filters(
    info: &NotificationInfo,
    app_filter: &Option<String>,
    text_filter: &Option<String>,
) -> bool {
    if let Some(app) = app_filter {
        if !info.app_name.to_lowercase().contains(app) {
            return false;
        }
    }
    if let Some(text) = text_filter {
        let haystack = format!(
            "{} {} {}",
            info.title,
            info.body.as_deref().unwrap_or(""),
            info.app_name
        )
        .to_lowercase();
        if !haystack.contains(text) {
            return false;
        }
    }
    true
}

#[cfg(not(target_os = "macos"))]
fn list_from_nc(_filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError> {
    Err(AdapterError::not_supported("list_notifications"))
}
