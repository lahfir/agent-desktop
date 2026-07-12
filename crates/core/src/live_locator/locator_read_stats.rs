use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LocatorReadStats {
    pub observation_attempts: u64,
    pub attribute_batches: u64,
    pub attributes_requested: u64,
    pub child_reads: u64,
    pub action_reads: u64,
    pub fallback_reads: u64,
    pub cannot_complete: u64,
    pub native_read_failures: u64,
    pub deadline_exhausted: u64,
}
