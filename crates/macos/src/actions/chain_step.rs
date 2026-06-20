use agent_desktop_core::{action::MouseButton, error::AdapterError};

use crate::tree::AXElement;

pub(crate) enum ChainStep {
    Action(&'static str),
    SetBool {
        attr: &'static str,
        value: bool,
    },
    SetDynamic {
        attr: &'static str,
    },
    FocusThenSetDynamic {
        attr: &'static str,
    },
    /// Converges a stepper/slider to the dynamic target value via repeated
    /// AXIncrement/AXDecrement actions, for controls whose AXValue is not
    /// directly settable.
    IncrementToDynamic,
    FocusThenClearByKeyboard,
    ChildActions {
        actions: &'static [&'static str],
        limit: usize,
    },
    AncestorActions {
        actions: &'static [&'static str],
        limit: usize,
    },
    Custom {
        label: &'static str,
        func: fn(&AXElement) -> Result<bool, AdapterError>,
    },
    /// Like `Custom`, for steps that poll for a settled state and must cap
    /// that settle wait to the chain's remaining deadline budget.
    CustomWithDeadline {
        label: &'static str,
        func: fn(&AXElement, Option<std::time::Instant>) -> Result<bool, AdapterError>,
    },
    CGClick {
        button: MouseButton,
        count: u32,
    },
}
