use agent_desktop_core::{
    action::ActionResult,
    error::{AdapterError, ErrorCode},
    notification::{NotificationFilter, NotificationInfo},
};

use super::NcSession;

pub fn dismiss_notification(
    index: usize,
    app_filter: Option<&str>,
) -> Result<NotificationInfo, AdapterError> {
    let session = NcSession::open()?;
    let result = dismiss_impl(index, app_filter);
    session.close()?;
    result
}

pub fn notification_action(index: usize, action_name: &str) -> Result<ActionResult, AdapterError> {
    let session = NcSession::open()?;
    let result = action_impl(index, action_name);
    session.close()?;
    result
}

#[cfg(target_os = "macos")]
fn dismiss_impl(index: usize, app_filter: Option<&str>) -> Result<NotificationInfo, AdapterError> {
    use crate::actions::ax_helpers::try_ax_action;
    use crate::tree::{copy_ax_array, copy_string_attr};
    use accessibility_sys::{kAXChildrenAttribute, kAXRoleAttribute};

    let filter = build_filter(app_filter);
    let entries = super::list::list_entries(&filter)?;

    let entry = entries
        .into_iter()
        .find(|e| e.info.index == index)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;

    let info = entry.info;

    hover_over(&entry.element)?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let children = copy_ax_array(&entry.element, kAXChildrenAttribute).unwrap_or_default();
    let close_btn = children.iter().find(|c| {
        if copy_string_attr(c, kAXRoleAttribute).as_deref() != Some("AXButton") {
            return false;
        }
        let name = copy_string_attr(c, "AXTitle")
            .or_else(|| copy_string_attr(c, "AXDescription"))
            .unwrap_or_default()
            .to_lowercase();
        name.contains("close") || name.contains("clear") || name.contains("dismiss")
    });

    if let Some(btn) = close_btn {
        if !try_ax_action(btn, "AXPress") {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Failed to press close button on notification",
            ));
        }
    } else if !try_ax_action(&entry.element, "AXPress") {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "No close button found and AXPress on notification group failed",
        ));
    }

    Ok(info)
}

#[cfg(target_os = "macos")]
fn action_impl(index: usize, action_name: &str) -> Result<ActionResult, AdapterError> {
    use crate::actions::ax_helpers::try_ax_action;
    use crate::tree::{copy_ax_array, copy_string_attr};
    use accessibility_sys::{kAXChildrenAttribute, kAXRoleAttribute};

    let filter = NotificationFilter::default();
    let entries = super::list::list_entries(&filter)?;

    let entry = entries
        .into_iter()
        .find(|e| e.info.index == index)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;

    let children = copy_ax_array(&entry.element, kAXChildrenAttribute).unwrap_or_default();
    let action_lower = action_name.to_lowercase();
    let action_btn = children.iter().find(|c| {
        if copy_string_attr(c, kAXRoleAttribute).as_deref() != Some("AXButton") {
            return false;
        }
        let name = copy_string_attr(c, "AXTitle")
            .or_else(|| copy_string_attr(c, "AXDescription"))
            .unwrap_or_default();
        name.to_lowercase() == action_lower
    });

    let btn = action_btn.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "Action '{}' not found on notification {}",
                action_name, index
            ),
        )
    })?;

    if !try_ax_action(btn, "AXPress") {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "Failed to press '{}' button on notification {}",
                action_name, index
            ),
        ));
    }

    Ok(ActionResult::new(action_name))
}

#[cfg(target_os = "macos")]
fn hover_over(el: &crate::tree::AXElement) -> Result<(), AdapterError> {
    use crate::tree::read_bounds;
    use agent_desktop_core::action::{MouseButton, MouseEvent, MouseEventKind, Point};

    let bounds = read_bounds(el)
        .ok_or_else(|| AdapterError::internal("Cannot read notification bounds for hover"))?;

    crate::input::mouse::synthesize_mouse(MouseEvent {
        kind: MouseEventKind::Move,
        point: Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        },
        button: MouseButton::Left,
    })
}

fn build_filter(app_filter: Option<&str>) -> NotificationFilter {
    NotificationFilter {
        app: app_filter.map(String::from),
        ..Default::default()
    }
}

#[cfg(not(target_os = "macos"))]
fn dismiss_impl(
    _index: usize,
    _app_filter: Option<&str>,
) -> Result<NotificationInfo, AdapterError> {
    Err(AdapterError::not_supported("dismiss_notification"))
}

#[cfg(not(target_os = "macos"))]
fn action_impl(_index: usize, _action_name: &str) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("notification_action"))
}
