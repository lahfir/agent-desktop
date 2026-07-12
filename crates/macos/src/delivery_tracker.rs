use agent_desktop_core::{AdapterError, DeliverySemantics};

#[derive(Clone, Copy, Default)]
pub(crate) struct DeliveryTracker {
    delivered_units: usize,
}

impl DeliveryTracker {
    pub(crate) fn from_delivered_units(delivered_units: usize) -> Self {
        Self { delivered_units }
    }

    pub(crate) fn mark_delivered(&mut self) {
        self.delivered_units = self.delivered_units.saturating_add(1);
    }

    pub(crate) fn delivered_units(self) -> usize {
        self.delivered_units
    }

    pub(crate) fn annotate(self, error: AdapterError) -> AdapterError {
        let disposition = match error.disposition {
            DeliverySemantics::DeliveryUncertain
            | DeliverySemantics::DeliveredUnverified
            | DeliverySemantics::DeliveredVerified => error.disposition,
            DeliverySemantics::Unknown | DeliverySemantics::NotDelivered => {
                if self.delivered_units == 0 {
                    DeliverySemantics::not_delivered()
                } else {
                    DeliverySemantics::delivered_unverified()
                }
            }
        };
        error.with_disposition(disposition)
    }

    pub(crate) fn uncertain(error: AdapterError) -> AdapterError {
        error.with_disposition(DeliverySemantics::uncertain())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_tracker_changes_retry_semantics_after_first_post() {
        let before = DeliveryTracker::default().annotate(AdapterError::internal("before"));
        let mut tracker = DeliveryTracker::default();
        tracker.mark_delivered();
        let after = tracker.annotate(AdapterError::internal("after"));

        assert_eq!(before.disposition, DeliverySemantics::not_delivered());
        assert_eq!(after.disposition, DeliverySemantics::delivered_unverified());
    }

    #[test]
    fn nested_partial_delivery_is_never_downgraded_by_an_outer_tracker() {
        for disposition in [
            DeliverySemantics::uncertain(),
            DeliverySemantics::delivered_unverified(),
            DeliverySemantics::delivered_verified(),
        ] {
            let inner = AdapterError::internal("partial key pair").with_disposition(disposition);
            let annotated = DeliveryTracker::default().annotate(inner);

            assert_eq!(annotated.disposition, disposition);
            assert_eq!(
                annotated.disposition.retry(),
                agent_desktop_core::RetryDisposition::Unsafe
            );
        }
    }
}
