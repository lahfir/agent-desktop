use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ObservationRoot, SnapshotSurface};
use serde_json::json;

use super::automation::root_from_hwnd;
use super::chromium;
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::properties::read_one;
#[cfg(target_os = "windows")]
use super::property_ids::TreeProperty;
use crate::system::window_enum::enumerate_top_level;
use crate::system::window_ops::passes_filter;

/// Resolves the element an observation root names, per-surface (`Window`
/// resolves the named window directly; `Focused` composes the focused-only
/// inventory).
///
/// `Surface` semantics live here: only the surfaces this adapter advertises
/// are handled (core validates the requested surface before the adapter is
/// called, so an unsupported surface never reaches this match).
pub(crate) fn surface_root(
    root: ObservationRoot<'_>,
    surface: SnapshotSurface,
    deadline: Deadline,
) -> Result<UIAElement, AdapterError> {
    match (root, surface) {
        (ObservationRoot::Window(window), SnapshotSurface::Window) => {
            let handle = window_hwnd(&window.id)?;
            root_from_hwnd(handle, deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::Focused) => {
            let handle = focused_hwnd_of(&window.id)?;
            root_from_hwnd(handle, deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::Sheet) => {
            let handle = focused_hwnd_of(&window.id)?;
            let element = root_from_hwnd(handle, deadline)?;
            if window_is_modal_sheet(&element, chromium::is_chromium_root(&element)) {
                Ok(element)
            } else {
                Err(AdapterError::new(
                    ErrorCode::WindowNotFound,
                    "The focused window is not a modal sheet",
                ))
            }
        }
        (ObservationRoot::Element { handle, .. }, _) => {
            super::element::uia_element(handle).cloned().map_err(|_| {
                AdapterError::new(
                    ErrorCode::StaleRef,
                    "Element root handle is invalid for this platform",
                )
                .with_details(json!({ "kind": "element_root_wrong_payload" }))
            })
        }
        (ObservationRoot::Window(_), _) => Err(AdapterError::not_supported("surface")),
    }
}

fn window_hwnd(id: &str) -> Result<isize, AdapterError> {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<isize>().ok())
        .filter(|handle| *handle > 0)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Malformed window identifier"))
}

/// The focused window's HWND, found through the focused-only window inventory
/// the same way `focused_window` composes it.
fn focused_hwnd_of(expected: &str) -> Result<isize, AdapterError> {
    let mut focused = None;
    enumerate_top_level(|window| {
        if !passes_filter(&window) {
            return true;
        }
        let is_focused = {
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                unsafe { GetForegroundWindow() == window.handle }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = &window;
                false
            }
        };
        if is_focused {
            focused = Some(window.handle as isize);
            false
        } else {
            true
        }
    })?;
    let handle = focused.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::WindowNotFound,
            "There is no focused window on this desktop",
        )
    })?;
    if format!("w-{handle}") != expected {
        return Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "The requested focused window is no longer the focused window",
        ));
    }
    Ok(handle)
}

/// Whether a window is a modal sheet surface, tested from the window's own
/// `WindowIsModal` property **before** any child is consulted (the macOS
/// window-is-surface pattern from `crates/macos/src/tree/surfaces.rs`).
///
/// This classifies a Chromium modal as a `Sheet` surface, making it reachable
/// via the sheet surface.
#[cfg(target_os = "windows")]
pub(crate) fn window_is_modal_sheet(root: &UIAElement, _chromium: bool) -> bool {
    matches!(
        read_one(root, TreeProperty::WindowIsModal).flag(),
        Some(true)
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn window_is_modal_sheet(_root: &UIAElement, _chromium: bool) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn a_malformed_window_id_is_rejected_before_the_platform_is_reached() {
        let error = window_hwnd("not-a-window").expect_err("must reject");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
    }

    /// `window_is_modal_sheet` reads the property with no provider; an absent
    /// or unknown read is not a sheet.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn an_absent_window_modal_read_is_not_a_sheet() {
        use super::super::element::{CannedElement, UIAElement};
        let element = UIAElement::from(CannedElement);
        assert!(!window_is_modal_sheet(&element, true));
    }

    /// The shipped predicate, on the lane that runs it.
    ///
    /// The non-Windows arm above drives a stub whose body is `false`, so it
    /// answers correctly for its own reasons and says nothing about the real
    /// read. This drives the real one against a live top-level window that is
    /// not modal, and asserts the provider's own answer first: with the
    /// provider confirmed to be reporting `false`, a predicate that ignored
    /// the read or inverted its comparison would classify an ordinary window
    /// as a `Sheet` surface and fail here.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_live_non_modal_window_is_not_classified_as_a_sheet() {
        use super::super::fixture::{HostedFixture, ensure_test_apartment};
        use super::super::walker_fake::deadline;

        ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("the fixture spawns");
        let root = root_from_hwnd(fixture.handle(), deadline()).expect("the fixture window roots");

        assert_eq!(
            read_one(&root, TreeProperty::WindowIsModal).flag(),
            Some(false),
            "the provider must answer this read for the classification below to be tested"
        );
        assert!(!window_is_modal_sheet(&root, false));
        assert!(
            !window_is_modal_sheet(&root, true),
            "the chromium flag is not consulted by this classification"
        );
    }
}
