//! The located menu element for the `Menu` surface arm: [`uia_menu_reachable`]'s
//! source-B shape returning the element instead of a bool, split out of
//! `menu_state.rs` on the 400-line file cap - the same purely mechanical
//! split `menu_state_multi.rs` is - with the predicate it answers shared from
//! there rather than duplicated.

use agent_desktop_core::{AdapterError, Deadline, ProcessId};

/// The located menu element plus the window it was found under, so the
/// observation can root at the element's own window when it carries one and
/// at the probed window when it does not.
#[cfg(target_os = "windows")]
pub(crate) struct MenuLocation {
    pub(crate) element: uiautomation::UIElement,
    pub(crate) window: isize,
}

/// The non-Windows shape of [`MenuLocation`]: no element can be located on a
/// lane with no UI Automation provider, so only the module's stub ever names
/// the type.
#[cfg(not(target_os = "windows"))]
pub(crate) struct MenuLocation {
    pub(crate) window: isize,
}

#[cfg(target_os = "windows")]
impl MenuLocation {
    /// The handle the observation roots at: a classic popup menu is its own
    /// `#32768` window and the element reports that handle, while a menu
    /// element with no native window of its own roots at the window it was
    /// located under.
    pub(crate) fn root_handle(&self) -> isize {
        self.element
            .get_native_window_handle()
            .ok()
            .map(|handle| -> isize { handle.into() })
            .filter(|handle| *handle != 0)
            .unwrap_or(self.window)
    }
}

#[cfg(not(target_os = "windows"))]
impl MenuLocation {
    /// No element is ever located on a lane with no UI Automation provider,
    /// so the only handle the type could name is the probed window's.
    pub(crate) fn root_handle(&self) -> isize {
        self.window
    }
}

/// The `Menu` surface arm, resolving the sources the detector answers with
/// so a wait and the snapshot that follows it agree wherever they can.
///
/// Source B (a menu-family element under a tool window) and source C (a
/// Chromium DOM menu) are both resolved here, because both name an element
/// this can root at.
///
/// **Source A cannot be, and this is the one place the two predicates do
/// diverge.** `classic_menu_mode_active` reads `GetGUIThreadInfo`'s
/// menu-mode flag, which reports that a classic Win32 menu is up without
/// naming any element: there is nothing to return. A `surface-appeared`
/// wait therefore succeeds for such a menu while `snapshot --surface menu`
/// reports it cannot be rooted. The previous wording claimed the two could
/// never disagree, which was false for exactly this case; the surface
/// arm's refusal now names it rather than reading as "no menu is open".
#[cfg(target_os = "windows")]
pub(crate) fn locate_menu(
    pid: ProcessId,
    deadline: Deadline,
) -> Result<Option<MenuLocation>, AdapterError> {
    super::ensure_budget(deadline)?;
    super::ensure_process_exists(pid)?;
    super::ensure_budget(deadline)?;
    let candidates = super::tool_window_candidates(pid)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let client =
        crate::tree::automation::automation_client().map_err(super::narrow_to_permitted_codes)?;
    let condition = super::menu_family_condition(&client)?;
    for (index, handle) in candidates.into_iter().enumerate() {
        if index > 0 {
            super::ensure_budget(deadline)?;
        }
        if let Some(element) = super::probe_candidate_element(&client, &condition, handle)? {
            return Ok(Some(MenuLocation {
                element,
                window: handle,
            }));
        }
    }
    locate_chromium(pid, deadline)
}

#[cfg(target_os = "windows")]
fn locate_chromium(
    pid: ProcessId,
    deadline: Deadline,
) -> Result<Option<MenuLocation>, AdapterError> {
    Ok(super::chromium::locate_chromium_dom_menu(pid, deadline)?
        .map(|(element, window)| MenuLocation { element, window }))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn locate_menu(
    _pid: ProcessId,
    deadline: Deadline,
) -> Result<Option<MenuLocation>, AdapterError> {
    super::ensure_budget(deadline)?;
    Ok(None)
}
