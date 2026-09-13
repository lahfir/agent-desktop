//! Test-only support for the live Action Center scenarios: stages one
//! synthetic toast under the Windows PowerShell AUMID - the exact staging the
//! notification-shape probe measured working on this host (A26-3), carrying
//! only the synthetic literals this suite owns - and sweeps the center clean
//! on every exit path.
//!
//! The sweep dismisses everything the center carries, which is the probe's own
//! cleanup protocol on this box: the center is machine-global test state and
//! this suite holds the shell lock for its whole body, so a clean slate at
//! every test boundary is what keeps the state assertions deterministic.

use std::time::Duration;

use agent_desktop_core::{
    Deadline, InteractionPolicy, NotificationFilter, NotificationInfo, SnapshotSurface,
};

use crate::notifications::actions;
use crate::notifications::list::list_infos;
use crate::system::shell_surface_kinds::MAIN_LIST_VIEW;
use crate::system::shell_surface_open::close_surface;

pub(crate) const TOAST_TITLE: &str = "agent-desktop-probe-notification";
pub(crate) const TOAST_BODY: &str = "agent-desktop-probe-notification-body";
pub(crate) const TOAST_TITLE_SECOND: &str = "agent-desktop-probe-notification-2";
pub(crate) const TOAST_BODY_SECOND: &str = "agent-desktop-probe-notification-body-2";

const TOAST_AUMID: &str =
    "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How long the reader is given to catch up to a toast the independent walk
/// already sees, before the gap is judged a reader regression.
const READER_SETTLE_MS: u64 = 3_000;

fn stage_toast(title: &str, body: &str) -> Result<(), String> {
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null; \
         $xml = New-Object Windows.Data.Xml.Dom.XmlDocument; \
         $xml.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{title}</text><text>{body}</text></binding></visual></toast>'); \
         $toast = New-Object Windows.UI.Notifications.ToastNotification($xml); \
         $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{TOAST_AUMID}'); \
         $notifier.Show($toast)"
    );
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|error| format!("powershell could not be started: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "the shell refused the staged toast: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Stages one synthetic toast and sweeps the center clean when dropped, on
/// every path out of the test body.
pub(crate) struct StagedToast;

impl StagedToast {
    pub(crate) fn stage() -> Self {
        Self::stage_with(TOAST_TITLE, TOAST_BODY)
    }

    pub(crate) fn stage_with(title: &str, body: &str) -> Self {
        if let Err(error) = stage_toast(title, body) {
            panic!("the synthetic toast could not be staged: {error}");
        }
        Self
    }
}

impl Drop for StagedToast {
    fn drop(&mut self) {
        clear_center(Deadline::detached_after(8_000).expect("cleanup deadline"));
    }
}

/// Dismisses whatever the center carries, best effort - a cleanup that cannot
/// report must not fail a test whose assertions already ran.
pub(crate) fn clear_center(deadline: Deadline) {
    let _ = actions::dismiss_all(None, InteractionPolicy::headed(), deadline);
}

/// Closes the Action Center when dropped, for tests that arrange the surface's
/// state themselves and must not leak it to the next test on a failure path.
pub(crate) struct CloseCenterOnDrop;

impl Drop for CloseCenterOnDrop {
    fn drop(&mut self) {
        let _ = close_surface(
            SnapshotSurface::ActionCenter,
            Deadline::detached_after(8_000).expect("cleanup deadline"),
        );
    }
}

/// Polls the listing until the staged toast is observable in the center and
/// returns the listing that carried it, or `None` when the budget expires
/// without the toast ever appearing - a desktop whose staging the shell never
/// lands, which the caller classifies as a declined precondition instead of
/// this wait asserting on. A toast lands in the center a short, variable time
/// after the shell accepts it, so the wait is on the value itself rather than
/// on a fixed sleep.
///
/// The wait is judged by two observers. The product reader answers the
/// caller's question - the listing that carried the toast; the independent
/// walk ([`independent_walk_sees_toast`]) answers whether the held center
/// carries the toast at all, read straight off the tree. When the walk sees
/// the toast and the reader still does not report it across a short bounded
/// re-poll, the reader is regressed and this panics: a reader regression
/// must fail a test, never expire into a loud skip.
///
/// This variant polls inside a center the caller holds open, because the
/// measured staging behaviour on this host is that a toast joins the center
/// only while the center is open and leaves it at the next close: a poll that
/// closed and reopened the center would evict the very entry it waits for.
pub(crate) fn wait_until_listed_held(
    hwnd: isize,
    deadline: Deadline,
) -> Option<Vec<NotificationInfo>> {
    let filter = NotificationFilter::default();
    loop {
        if let Ok(listed) = list_infos(&filter, hwnd, deadline) {
            if listed.iter().any(|info| info.title == TOAST_TITLE) {
                return Some(listed);
            }
        }
        if independent_walk_sees_toast(hwnd, deadline) {
            if let Some(listed) = product_reader_catches_up(hwnd) {
                return Some(listed);
            }
            panic!(
                "the product reader misses an entry the independent walk observes: the \
                 held center carries the staged toast but the listing never reports it"
            );
        }
        if deadline.is_expired() {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// A short bounded re-poll of the product reader alone. The independent walk
/// has already seen the staged toast, so the only question left is whether
/// the reader catches up to the tree inside this window - the toast may have
/// landed between the reader's last poll and the walk - or is proven to miss
/// an entry the desktop carries. Its budget is detached from the caller's:
/// the reader's honesty is not the wait's remaining latency.
fn product_reader_catches_up(hwnd: isize) -> Option<Vec<NotificationInfo>> {
    let filter = NotificationFilter::default();
    let deadline = Deadline::detached_after(READER_SETTLE_MS).expect("re-poll deadline");
    loop {
        if let Ok(listed) = list_infos(&filter, hwnd, deadline) {
            if listed.iter().any(|info| info.title == TOAST_TITLE) {
                return Some(listed);
            }
        }
        if deadline.is_expired() {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The independent observation the staging wait is judged against: a raw UIA
/// walk of the held center's hwnd that locates the notification list by its
/// measured `MainListView` structure, enumerates its `ListItem` descendants
/// by control type, and matches the staged toast by a descendant's raw
/// `Name` - the title is the fixed synthetic literal this suite owns, so an
/// exact match is safe in tests. Nothing is read through the product reader,
/// so a reader regression cannot hide behind the oracle that watches it.
///
/// Every read failure reads as "not seen": the oracle speaks only when the
/// tree it can actually observe carries the toast, never from a search that
/// could not run.
fn independent_walk_sees_toast(hwnd: isize, deadline: Deadline) -> bool {
    use uiautomation::types::{ControlType, TreeScope, UIProperty};
    use uiautomation::variants::Variant;

    let Ok(root) = crate::tree::automation::root_from_hwnd(hwnd, deadline) else {
        return false;
    };
    let Ok(client) = crate::tree::automation::automation_client() else {
        return false;
    };
    let list_condition = match client.create_property_condition(
        UIProperty::AutomationId,
        Variant::from(MAIN_LIST_VIEW),
        None,
    ) {
        Ok(condition) => condition,
        Err(_) => return false,
    };
    let Ok(list) = root.0.find_first(TreeScope::Descendants, &list_condition) else {
        return false;
    };
    let items_condition = match client.create_property_condition(
        UIProperty::ControlType,
        Variant::from(ControlType::ListItem as i32),
        None,
    ) {
        Ok(condition) => condition,
        Err(_) => return false,
    };
    let Ok(items) = list.find_all(TreeScope::Descendants, &items_condition) else {
        return false;
    };
    let title_condition = match client.create_property_condition(
        UIProperty::Name,
        Variant::from(TOAST_TITLE),
        None,
    ) {
        Ok(condition) => condition,
        Err(_) => return false,
    };
    items.iter().any(|item| {
        item.find_first(TreeScope::Descendants, &title_condition)
            .is_ok()
    })
}

/// The same wait, until the held center carries `count` entries.
pub(crate) fn wait_until_count_held(
    hwnd: isize,
    count: usize,
    deadline: Deadline,
) -> Vec<NotificationInfo> {
    let filter = NotificationFilter::default();
    loop {
        match list_infos(&filter, hwnd, deadline) {
            Ok(listed) if listed.len() >= count => return listed,
            Ok(_) => {}
            Err(_) => {}
        }
        assert!(
            !deadline.is_expired(),
            "the center never carried {count} entries"
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}
