use super::{CursorOverlayConfig, CursorPhase};
use crate::{AdapterError, ErrorCode, Point, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CursorOverlayInstruction {
    destination: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drag_from: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    click: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<Rect>,
    #[serde(default)]
    phase: CursorPhase,
}

impl CursorOverlayInstruction {
    pub fn new(
        destination: Point,
        config: &CursorOverlayConfig,
        click: bool,
    ) -> Result<Self, AdapterError> {
        destination.validate()?;
        if !config.is_enabled() {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "Agent cursor instruction requires enabled presentation",
            ));
        }
        Ok(Self {
            destination,
            drag_from: None,
            label: config.label().map(str::to_owned),
            click,
            target: None,
            phase: CursorPhase::Travel,
        })
    }

    pub const fn with_phase(mut self, phase: CursorPhase) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_drag_from(mut self, drag_from: Option<Point>) -> Self {
        self.drag_from = drag_from;
        self
    }

    pub const fn drag_from(&self) -> Option<&Point> {
        self.drag_from.as_ref()
    }

    pub const fn phase(&self) -> CursorPhase {
        self.phase
    }

    pub fn with_target(mut self, target: Option<Rect>) -> Self {
        self.target =
            target.filter(|rect| rect.validate().is_ok() && rect.width > 0.0 && rect.height > 0.0);
        self
    }

    pub const fn target(&self) -> Option<&Rect> {
        self.target.as_ref()
    }

    pub fn validate(&self) -> Result<(), AdapterError> {
        self.destination.validate()?;
        if let Some(drag_from) = &self.drag_from {
            drag_from.validate()?;
        }
        Ok(())
    }

    pub fn destination(&self) -> &Point {
        &self.destination
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub const fn is_click(&self) -> bool {
        self.click
    }
}
