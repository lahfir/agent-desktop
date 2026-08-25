use super::CursorOverlayConfig;
use crate::{AdapterError, ErrorCode, Point, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CursorOverlayInstruction {
    destination: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    click: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<Rect>,
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
            label: config.label().map(str::to_owned),
            click,
            target: None,
        })
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
        self.destination.validate()
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
