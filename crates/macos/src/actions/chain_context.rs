pub(crate) struct ChainContext<'a> {
    pub(crate) dynamic_value: Option<&'a str>,
    pub(crate) deadline: Option<std::time::Instant>,
}

impl<'a> ChainContext<'a> {
    /// Pins the chain's resolved deadline so every step — notably the
    /// `IncrementToDynamic` loop — observes the same budget the chain
    /// enforces between steps. Callers construct contexts with
    /// `deadline: None`; the chain owns resolving that into an instant.
    pub(crate) fn with_deadline(&self, deadline: std::time::Instant) -> ChainContext<'a> {
        ChainContext {
            dynamic_value: self.dynamic_value,
            deadline: Some(deadline),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChainContext;
    use std::time::{Duration, Instant};

    #[test]
    fn with_deadline_pins_the_instant_and_keeps_the_dynamic_value() {
        let base = ChainContext {
            dynamic_value: Some("42"),
            deadline: None,
        };
        let deadline = Instant::now() + Duration::from_secs(1);

        let effective = base.with_deadline(deadline);

        assert_eq!(effective.dynamic_value, Some("42"));
        assert_eq!(effective.deadline, Some(deadline));
    }
}
