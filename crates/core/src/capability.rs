use crate::action::Action;

pub const CLICK: &str = "Click";
pub const RIGHT_CLICK: &str = "RightClick";
pub const SET_VALUE: &str = "SetValue";
pub const SET_FOCUS: &str = "SetFocus";
pub const EXPAND: &str = "Expand";
pub const COLLAPSE: &str = "Collapse";
pub const SELECT: &str = "Select";
pub const TOGGLE: &str = "Toggle";
pub const SCROLL: &str = "Scroll";
pub const SCROLL_TO: &str = "ScrollTo";
pub const PRESS_KEY: &str = "PressKey";
pub const KEY_DOWN: &str = "KeyDown";
pub const KEY_UP: &str = "KeyUp";
pub const TYPE_TEXT: &str = "TypeText";
pub const HOVER: &str = "Hover";
pub const DRAG: &str = "Drag";
pub const CHECK: &str = "Check";
pub const UNCHECK: &str = "Uncheck";

pub const CHECKED_APPLICABILITY: &[&str] = &[TOGGLE, CHECK, UNCHECK];
pub const EXPANDED_APPLICABILITY: &[&str] = &[EXPAND, COLLAPSE];

pub fn for_action(action: &Action) -> &'static [&'static str] {
    match action {
        Action::Click | Action::DoubleClick | Action::TripleClick => &[CLICK],
        Action::RightClick => &[RIGHT_CLICK],
        Action::SetValue(_) | Action::Clear => &[SET_VALUE],
        Action::SetFocus => &[SET_FOCUS],
        Action::Expand => &[EXPAND],
        Action::Collapse => &[COLLAPSE],
        Action::Select(_) => &[SELECT, CLICK],
        Action::Toggle => &[TOGGLE, CLICK],
        Action::Check | Action::Uncheck => &[TOGGLE, CLICK],
        Action::Scroll(_, _) => &[SCROLL, SCROLL_TO],
        Action::ScrollTo => &[SCROLL_TO],
        Action::PressKey(_) => &[PRESS_KEY],
        Action::KeyDown(_) => &[KEY_DOWN],
        Action::KeyUp(_) => &[KEY_UP],
        Action::TypeText(_) => &[TYPE_TEXT, SET_VALUE],
        Action::Hover => &[HOVER],
        Action::Drag(_) => &[DRAG],
    }
}

pub fn defaults_for_role(role: &str) -> Vec<String> {
    let capabilities: &[&str] = match role {
        "button" | "link" | "menuitem" | "tab" | "radiobutton" => &[CLICK],
        "textfield" | "incrementor" => &[CLICK, SET_VALUE, SET_FOCUS],
        "checkbox" => &[CLICK, TOGGLE],
        "combobox" => &[CLICK, SELECT],
        "treeitem" => &[CLICK, EXPAND, COLLAPSE],
        "slider" => &[SET_VALUE],
        "cell" => &[CLICK],
        _ => &[CLICK],
    };
    capabilities
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

pub fn contains(actions: &[String], capability: &str) -> bool {
    actions.iter().any(|action| action == capability)
}

pub fn contains_any(actions: &[String], capabilities: &[&str]) -> bool {
    capabilities
        .iter()
        .any(|capability| contains(actions, capability))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Direction, KeyCombo};

    #[test]
    fn action_capabilities_are_declared_in_one_place() {
        assert_eq!(for_action(&Action::Click), &[CLICK]);
        assert_eq!(for_action(&Action::RightClick), &[RIGHT_CLICK]);
        assert_eq!(for_action(&Action::SetValue("x".into())), &[SET_VALUE]);
        assert_eq!(for_action(&Action::Clear), &[SET_VALUE]);
        assert_eq!(
            for_action(&Action::Scroll(Direction::Down, 1)),
            &[SCROLL, SCROLL_TO]
        );
        assert_eq!(
            for_action(&Action::PressKey(KeyCombo {
                key: "A".into(),
                modifiers: vec![],
            })),
            &[PRESS_KEY]
        );
    }

    #[test]
    fn role_defaults_are_declared_in_one_place() {
        assert_eq!(defaults_for_role("button"), strings(&[CLICK]));
        assert_eq!(
            defaults_for_role("textfield"),
            strings(&[CLICK, SET_VALUE, SET_FOCUS])
        );
        assert_eq!(
            defaults_for_role("treeitem"),
            strings(&[CLICK, EXPAND, COLLAPSE])
        );
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }
}
