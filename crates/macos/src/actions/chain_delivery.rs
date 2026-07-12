#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    NotDelivered,
    SatisfiedNoDelivery,
    DeliveredUnverified,
    DeliveredVerified,
}

impl DeliveryOutcome {
    pub(crate) fn from_delivery(delivered: bool, verified: bool) -> Self {
        match (delivered, verified) {
            (false, _) => Self::NotDelivered,
            (true, false) => Self::DeliveredUnverified,
            (true, true) => Self::DeliveredVerified,
        }
    }

    pub(crate) fn was_delivered(self) -> bool {
        matches!(self, Self::DeliveredUnverified | Self::DeliveredVerified)
    }

    pub(crate) fn was_verified(self) -> bool {
        matches!(self, Self::SatisfiedNoDelivery | Self::DeliveredVerified)
    }

    pub(crate) fn terminates_chain(self) -> bool {
        !matches!(self, Self::NotDelivered)
    }
}

#[cfg(test)]
mod tests {
    use super::DeliveryOutcome;

    #[test]
    fn delivery_and_verification_are_independent() {
        assert_eq!(
            DeliveryOutcome::from_delivery(false, true),
            DeliveryOutcome::NotDelivered
        );
        assert_eq!(
            DeliveryOutcome::from_delivery(true, false),
            DeliveryOutcome::DeliveredUnverified
        );
        assert_eq!(
            DeliveryOutcome::from_delivery(true, true),
            DeliveryOutcome::DeliveredVerified
        );
        assert!(DeliveryOutcome::SatisfiedNoDelivery.terminates_chain());
        assert!(!DeliveryOutcome::SatisfiedNoDelivery.was_delivered());
        assert!(DeliveryOutcome::SatisfiedNoDelivery.was_verified());
    }
}
