#[derive(Debug, Clone, PartialEq)]
pub enum LocatorField<T> {
    Known(T),
    Absent,
    Unknown,
}

impl<T> LocatorField<T> {
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Absent | Self::Unknown => None,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

impl LocatorField<String> {
    pub(crate) fn meaningful_string(&self) -> Option<String> {
        self.known().filter(|value| !value.is_empty()).cloned()
    }
}
