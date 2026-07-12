#[derive(Default)]
pub(crate) struct ChildReadStatus {
    pub(crate) attempts: u64,
    pub(crate) cannot_complete: u64,
    pub(crate) native_read_failures: u64,
    pub(crate) invalid_element: bool,
    pub(crate) api_disabled: bool,
    pub(crate) deadline_exhausted: bool,
    pub(crate) count_changed: bool,
    pub(crate) cursor_stalled: bool,
}

impl ChildReadStatus {
    pub(crate) fn merge(&mut self, other: Self) {
        self.attempts += other.attempts;
        self.cannot_complete += other.cannot_complete;
        self.native_read_failures += other.native_read_failures;
        self.invalid_element |= other.invalid_element;
        self.api_disabled |= other.api_disabled;
        self.deadline_exhausted |= other.deadline_exhausted;
        self.count_changed |= other.count_changed;
        self.cursor_stalled |= other.cursor_stalled;
    }
}
