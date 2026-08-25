use super::CursorOverlayConfig;
use crate::{AdapterError, ErrorCode, Point};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CursorOverlayInstruction {
    destination: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    click: bool,
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
        })
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
