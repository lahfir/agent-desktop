use super::LocatorMatchData;
use crate::refs::RefEntry;

pub struct LocatorMatch {
    pub data: LocatorMatchData,
    pub document_order: u32,
    pub(crate) entry: RefEntry,
}

impl LocatorMatch {
    pub fn into_entry(self) -> RefEntry {
        self.entry
    }
}
