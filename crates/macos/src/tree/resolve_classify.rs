use agent_desktop_core::{adapter::NativeHandle, error::AdapterError, refs::RefEntry};

use super::AXElement;
#[cfg(target_os = "macos")]
use super::attributes::copy_string_attr;
#[cfg(target_os = "macos")]
use super::element::resolve_element_name;
#[cfg(target_os = "macos")]
use super::resolve_bounds::bounds_match;

#[cfg(target_os = "macos")]
pub(super) fn classify_candidates(
    mut matches: Vec<AXElement>,
    entry: &RefEntry,
    source_window_verified: bool,
) -> Result<NativeHandle, AdapterError> {
    match matches.len() {
        0 => Err(AdapterError::element_not_found("element")),
        1 => {
            let candidate = matches.remove(0);
            if source_window_verified || verified_bounds_match(&candidate, entry) {
                retained_handle(candidate)
            } else {
                Err(AdapterError::element_not_found("element"))
            }
        }
        _ => classify_ambiguous_candidates(matches, entry),
    }
}

#[cfg(target_os = "macos")]
fn verified_bounds_match(candidate: &AXElement, entry: &RefEntry) -> bool {
    entry.bounds_hash.is_some() && bounds_match(candidate, entry)
}

#[cfg(target_os = "macos")]
fn classify_ambiguous_candidates(
    matches: Vec<AXElement>,
    entry: &RefEntry,
) -> Result<NativeHandle, AdapterError> {
    let mut bounds_matches: Vec<_> = matches
        .iter()
        .filter(|candidate| verified_bounds_match(candidate, entry))
        .cloned()
        .collect();
    if entry.bounds_hash.is_some() && bounds_matches.is_empty() {
        return Err(AdapterError::element_not_found("element"));
    }
    if bounds_matches.len() == 1 {
        return retained_handle(bounds_matches.remove(0));
    }
    let count = matches.len();
    Err(AdapterError::ambiguous_target(format!(
        "Ambiguous target: {count} candidates matched {}",
        identity_summary_for_message(entry)
    ))
    .with_details(serde_json::json!({
        "candidate_count": count,
        "role": entry.role,
        "name": entry.name,
        "description": entry.description,
        "source_app": entry.source_app,
        "source_window_id": entry.source_window_id,
        "source_window_title": entry.source_window_title,
        "candidates": candidate_summaries(&matches)
    })))
}

#[cfg(target_os = "macos")]
pub(super) fn identity_summary_for_message(entry: &RefEntry) -> String {
    format!(
        "role={}, name_chars={}, description_chars={}",
        entry.role,
        text_len(entry.name.as_deref()),
        text_len(entry.description.as_deref())
    )
}

#[cfg(target_os = "macos")]
fn text_len(value: Option<&str>) -> usize {
    value.unwrap_or("").chars().count()
}

#[cfg(target_os = "macos")]
fn retained_handle(candidate: AXElement) -> Result<NativeHandle, AdapterError> {
    use core_foundation::base::{CFRetain, CFTypeRef};
    if candidate.0.is_null() {
        #[cfg(test)]
        return Ok(NativeHandle::null());
        #[cfg(not(test))]
        return Err(AdapterError::element_not_found("element"));
    }
    unsafe { CFRetain(candidate.0 as CFTypeRef) };
    Ok(unsafe { NativeHandle::from_ptr(candidate.0 as *const _) })
}

#[cfg(target_os = "macos")]
fn candidate_summaries(matches: &[AXElement]) -> Vec<serde_json::Value> {
    matches
        .iter()
        .take(10)
        .enumerate()
        .map(|(index, element)| {
            let ax_role = copy_string_attr(element, accessibility_sys::kAXRoleAttribute);
            let (role, label) =
                crate::tree::roles::normalized_role_and_label(element, ax_role.as_deref());
            let name = label.or_else(|| resolve_element_name(element));
            let description = copy_string_attr(element, accessibility_sys::kAXDescriptionAttribute);
            let bounds = crate::tree::read_bounds(element);
            serde_json::json!({
                "index": index,
                "role": role,
                "name": name,
                "description": description,
                "bounds": bounds,
                "bounds_hash": bounds.as_ref().map(|bounds| bounds.bounds_hash())
            })
        })
        .collect()
}
