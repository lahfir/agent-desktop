//! One production caller. The scan must see it.

fn resolve(entry: &RefEntry) -> Result<Handle, AdapterError> {
    Err(AdapterError::stale_ref(&entry.ref_id))
}
