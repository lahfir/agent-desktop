use super::{CursorOverlayInstruction, CursorOverlayStyle, CursorPhase};
use crate::{AdapterError, ErrorCode, context::validate_session_id};
use serde::{Deserialize, Serialize};

pub const CURSOR_OVERLAY_GREETING: &str = "Hey, let's play with this computer!";

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CursorOverlayControl {
    Enable {
        session_id: String,
        label: String,
        #[serde(default)]
        style: CursorOverlayStyle,
    },
    Present {
        session_id: String,
        instruction: CursorOverlayInstruction,
        #[serde(default)]
        style: CursorOverlayStyle,
    },
    Hide {
        session_id: String,
    },
    Show {
        session_id: String,
    },
    Disable {
        session_id: String,
    },
}

impl CursorOverlayControl {
    /// The first frame of an overlay, carrying the caller's own label when
    /// they gave one.
    ///
    /// `None` falls back to the greeting, which is what an overlay enabled
    /// with nothing to say announces itself with. A caller who did supply a
    /// label must see it: silently replacing their words with the greeting
    /// makes the first thing anyone reads on screen a joke about a computer
    /// rather than what the agent is about to do, and no output tells them
    /// their label went nowhere.
    pub fn enable(session_id: String, label: Option<String>, style: CursorOverlayStyle) -> Self {
        Self::Enable {
            session_id,
            label: label.unwrap_or_else(|| CURSOR_OVERLAY_GREETING.into()),
            style,
        }
    }

    pub fn style(&self) -> Option<&CursorOverlayStyle> {
        match self {
            Self::Enable { style, .. } | Self::Present { style, .. } => Some(style),
            _ => None,
        }
    }

    pub fn present(session_id: String, instruction: CursorOverlayInstruction) -> Self {
        Self::present_with_style(session_id, instruction, CursorOverlayStyle::default())
    }

    pub fn present_with_style(
        session_id: String,
        instruction: CursorOverlayInstruction,
        style: CursorOverlayStyle,
    ) -> Self {
        Self::Present {
            session_id,
            instruction,
            style,
        }
    }

    pub fn disable(session_id: String) -> Self {
        Self::Disable { session_id }
    }

    pub fn hide(session_id: String) -> Self {
        Self::Hide { session_id }
    }

    pub fn show(session_id: String) -> Self {
        Self::Show { session_id }
    }

    pub fn validate(&self) -> Result<(), AdapterError> {
        validate_session_id(self.session_id()).map_err(|_| {
            AdapterError::new(ErrorCode::InvalidArgs, "Invalid cursor overlay session id")
        })?;
        if let Self::Present { instruction, .. } = self {
            instruction.validate()?;
        }
        Ok(())
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::Enable { session_id, .. }
            | Self::Present { session_id, .. }
            | Self::Hide { session_id }
            | Self::Show { session_id }
            | Self::Disable { session_id } => session_id,
        }
    }

    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Enable { label, .. } => Some(label),
            Self::Present { instruction, .. } => instruction.label(),
            Self::Hide { .. } | Self::Show { .. } | Self::Disable { .. } => None,
        }
    }

    pub const fn is_enable(&self) -> bool {
        matches!(self, Self::Enable { .. })
    }

    pub const fn is_disable(&self) -> bool {
        matches!(self, Self::Disable { .. })
    }

    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Hide { .. } | Self::Show { .. })
    }

    /// A travel control moves the cursor and carries no click effect. Every
    /// platform renderer must acknowledge it once the cursor lands, because the
    /// action waits for that acknowledgement before it dispatches.
    pub fn is_travel(&self) -> bool {
        self.instruction()
            .is_some_and(|instruction| instruction.phase() == CursorPhase::Travel)
    }

    pub const fn is_hide(&self) -> bool {
        matches!(self, Self::Hide { .. })
    }

    pub const fn is_show(&self) -> bool {
        matches!(self, Self::Show { .. })
    }

    pub fn instruction(&self) -> Option<&CursorOverlayInstruction> {
        match self {
            Self::Present { instruction, .. } => Some(instruction),
            _ => None,
        }
    }
}
