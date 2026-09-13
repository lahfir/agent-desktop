//! The case the brace tracking exists for: a gated item whose callers must be
//! skipped, followed by a real production caller that must still be counted.
//! Truncating the file at the first gate attribute would drop the caller below
//! it and the scan would under-count in silence.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gated_caller_is_not_production() {
        let _ = AdapterError::stale_ref("gated, must not count");
    }
}

fn production_caller_after_the_gated_item(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref(&entry.ref_id)
}
