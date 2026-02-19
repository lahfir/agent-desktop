use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    error::AppError,
    refs::{RefEntry, RefMap},
};

pub struct RefArgs {
    pub ref_id: String,
}

pub fn resolve_ref(
    ref_id: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<(RefEntry, NativeHandle), AppError> {
    validate_ref_id(ref_id)?;
    let refmap = RefMap::load().map_err(|_| AppError::stale_ref(ref_id))?;
    let entry = refmap.get(ref_id).ok_or_else(|| AppError::stale_ref(ref_id))?.clone();
    let handle = adapter.resolve_element(&entry)?;
    Ok((entry, handle))
}

pub fn validate_ref_id(ref_id: &str) -> Result<(), AppError> {
    let valid = ref_id.starts_with("@e")
        && ref_id.len() >= 3
        && ref_id.len() <= 12
        && ref_id[2..].chars().all(|c| c.is_ascii_digit());
    if !valid {
        return Err(AppError::invalid_input(
            format!("Invalid ref_id '{ref_id}': must match @e{{N}} where N is a positive integer"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_refs() {
        assert!(validate_ref_id("@e1").is_ok());
        assert!(validate_ref_id("@e14").is_ok());
        assert!(validate_ref_id("@e999").is_ok());
    }

    #[test]
    fn test_invalid_refs() {
        assert!(validate_ref_id("@").is_err());
        assert!(validate_ref_id("e1").is_err());
        assert!(validate_ref_id("@e").is_err());
        assert!(validate_ref_id("@e0abc").is_err());
        assert!(validate_ref_id("1").is_err());
        assert!(validate_ref_id("").is_err());
    }
}
