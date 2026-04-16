use crate::convert::string::c_to_str;
use crate::types::{AdAction, AdActionKind, AdDirection, AdKeyCombo, AdModifier};
use agent_desktop_core::action::{
    Action, Direction, DragParams as CoreDragParams, KeyCombo as CoreKeyCombo, Modifier,
    Point as CorePoint,
};

pub(crate) fn direction_from_c(d: AdDirection) -> Direction {
    match d {
        AdDirection::Up => Direction::Up,
        AdDirection::Down => Direction::Down,
        AdDirection::Left => Direction::Left,
        AdDirection::Right => Direction::Right,
    }
}

pub(crate) unsafe fn key_combo_from_c(k: &AdKeyCombo) -> Result<CoreKeyCombo, &'static str> {
    let key = c_to_str(k.key)
        .ok_or("key is null or invalid UTF-8")?
        .to_owned();
    let mut modifiers = Vec::new();
    if !k.modifiers.is_null() && k.modifier_count > 0 {
        let slice = std::slice::from_raw_parts(k.modifiers, k.modifier_count as usize);
        for m in slice {
            let modifier = match m {
                AdModifier::Cmd => Modifier::Cmd,
                AdModifier::Ctrl => Modifier::Ctrl,
                AdModifier::Alt => Modifier::Alt,
                AdModifier::Shift => Modifier::Shift,
            };
            modifiers.push(modifier);
        }
    }
    Ok(CoreKeyCombo { key, modifiers })
}

pub(crate) unsafe fn action_from_c(action: &AdAction) -> Result<Action, &'static str> {
    match action.kind {
        AdActionKind::Click => Ok(Action::Click),
        AdActionKind::DoubleClick => Ok(Action::DoubleClick),
        AdActionKind::RightClick => Ok(Action::RightClick),
        AdActionKind::TripleClick => Ok(Action::TripleClick),
        AdActionKind::SetFocus => Ok(Action::SetFocus),
        AdActionKind::Expand => Ok(Action::Expand),
        AdActionKind::Collapse => Ok(Action::Collapse),
        AdActionKind::Toggle => Ok(Action::Toggle),
        AdActionKind::Check => Ok(Action::Check),
        AdActionKind::Uncheck => Ok(Action::Uncheck),
        AdActionKind::ScrollTo => Ok(Action::ScrollTo),
        AdActionKind::Clear => Ok(Action::Clear),
        AdActionKind::Hover => Ok(Action::Hover),
        AdActionKind::SetValue => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::SetValue(text.to_owned()))
        }
        AdActionKind::Select => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::Select(text.to_owned()))
        }
        AdActionKind::TypeText => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::TypeText(text.to_owned()))
        }
        AdActionKind::Scroll => {
            let dir = direction_from_c(action.scroll.direction);
            Ok(Action::Scroll(dir, action.scroll.amount))
        }
        AdActionKind::PressKey => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::PressKey(combo))
        }
        AdActionKind::KeyDown => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::KeyDown(combo))
        }
        AdActionKind::KeyUp => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::KeyUp(combo))
        }
        AdActionKind::Drag => {
            let params = CoreDragParams {
                from: CorePoint {
                    x: action.drag.from.x,
                    y: action.drag.from.y,
                },
                to: CorePoint {
                    x: action.drag.to.x,
                    y: action.drag.to.y,
                },
                duration_ms: if action.drag.duration_ms == 0 {
                    None
                } else {
                    Some(action.drag.duration_ms)
                },
            };
            Ok(Action::Drag(params))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::{free_c_string, string_to_c};
    use crate::types::{AdDragParams, AdPoint, AdScrollParams};
    use std::ptr;

    fn make_scroll_params() -> AdScrollParams {
        AdScrollParams {
            direction: AdDirection::Down,
            amount: 3,
        }
    }

    fn make_key_combo() -> AdKeyCombo {
        AdKeyCombo {
            key: ptr::null(),
            modifiers: ptr::null(),
            modifier_count: 0,
        }
    }

    fn make_drag_params() -> AdDragParams {
        AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 0.0, y: 0.0 },
            duration_ms: 0,
        }
    }

    #[test]
    fn test_simple_action_roundtrip() {
        let action = AdAction {
            kind: AdActionKind::Click,
            text: ptr::null(),
            scroll: make_scroll_params(),
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::Click));
    }

    #[test]
    fn test_set_value_action() {
        let text = string_to_c("hello world");
        let action = AdAction {
            kind: AdActionKind::SetValue,
            text,
            scroll: make_scroll_params(),
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::SetValue(ref s) if s == "hello world"));
        unsafe { free_c_string(text as *mut _) };
    }

    #[test]
    fn test_scroll_action() {
        let action = AdAction {
            kind: AdActionKind::Scroll,
            text: ptr::null(),
            scroll: AdScrollParams {
                direction: AdDirection::Up,
                amount: 5,
            },
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::Scroll(Direction::Up, 5)));
    }
}
