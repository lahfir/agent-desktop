#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdPolicyKind {
    Headless = 0,
    FocusFallback = 1,
    Headed = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_are_abi_stable() {
        assert_eq!(AdPolicyKind::Headless as i32, 0);
        assert_eq!(AdPolicyKind::FocusFallback as i32, 1);
        assert_eq!(AdPolicyKind::Headed as i32, 2);
    }
}
