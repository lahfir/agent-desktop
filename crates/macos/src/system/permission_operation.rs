#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PermissionOperation {
    Accessibility,
    ScreenRecording,
}

impl PermissionOperation {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "accessibility" => Some(Self::Accessibility),
            "screen_recording" => Some(Self::ScreenRecording),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::ScreenRecording => "screen_recording",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_operation_is_a_closed_two_value_protocol() {
        assert_eq!(
            PermissionOperation::parse("accessibility"),
            Some(PermissionOperation::Accessibility)
        );
        assert_eq!(
            PermissionOperation::parse("screen_recording"),
            Some(PermissionOperation::ScreenRecording)
        );
        assert_eq!(PermissionOperation::parse("shell"), None);
    }
}
