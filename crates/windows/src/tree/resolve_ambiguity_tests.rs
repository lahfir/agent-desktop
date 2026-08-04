use super::*;
use agent_desktop_core::{ElementIdentifier, IdentifierKind, LocatorEvidence};

use std::ffi::c_void;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, MSG, PostThreadMessageW,
    SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage, WM_QUIT, WS_CHILD, WS_OVERLAPPEDWINDOW,
    WS_VISIBLE,
};

/// Application-derived text the host puts on the wire so a redaction
/// regression has something to leak. A marker reaching an error's message,
/// suggestion, platform detail or details is app text the resolver had no
/// right to echo.
const NAME_MARKER: &str = "zzduplicatenamezz";
const TITLE_MARKER: &str = "zzduplicatetitlezz";
const APP_MARKER: &str = "zzduplicateappzz.exe";

/// The control id both children share, so the UI Automation bridge hands
/// both the same `AutomationId` (A17-3 measured this as the reproducible way
/// an owned Win32 fixture presents a duplicate-evidence pair).
const SHARED_CONTROL_ID: usize = 31337;
const CONTROL_BORDER: u32 = 0x0080_0000;
const CONTROL_LEFT: i32 = 8;
const CONTROL_TOP: i32 = 8;
const CONTROL_WIDTH: i32 = 200;
const CONTROL_HEIGHT: i32 = 24;
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const WALKABLE_TIMEOUT: Duration = Duration::from_secs(20);
const WALKABLE_POLL: Duration = Duration::from_millis(100);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A window whose two children are indistinguishable to every resolution
/// tier: same class, same text, same shared control id, and the same
/// rectangle, so the bounds tie-break has two equal hashes to choose between
/// rather than none.
struct DuplicatePairWindow {
    handle: isize,
    thread_id: u32,
    host: Option<JoinHandle<()>>,
}

impl DuplicatePairWindow {
    fn create() -> Result<Self, String> {
        let (sender, receiver) = channel();
        let host = std::thread::spawn(move || host_duplicate_pair(sender));
        let (handle, thread_id) = receiver
            .recv_timeout(READY_TIMEOUT)
            .map_err(|_| String::from("the duplicate-pair host never reported a window"))?;
        let hosted = Self {
            handle,
            thread_id,
            host: Some(host),
        };
        await_walkable(handle)?;
        Ok(hosted)
    }
}

impl Drop for DuplicatePairWindow {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(host) = self.host.take() {
            let _ = host.join();
        }
    }
}

/// Creates the window and its duplicate pair on this thread and pumps until
/// told to quit.
///
/// The pump has to own the window and do nothing else: `ElementFromHandle`
/// sends `WM_GETOBJECT`, and a cross-thread `SendMessage` blocks until the
/// owning thread dispatches it, so a thread that both hosts the window and
/// waits on a UI Automation result deadlocks. Teardown posts `WM_QUIT` to the
/// thread rather than the window, so the window is destroyed by the thread
/// that created it.
fn host_duplicate_pair(ready: Sender<(isize, u32)>) {
    let class = wide("#32770");
    let title = wide(TITLE_MARKER);
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            2_100,
            2_100,
            420,
            220,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if window.is_null() {
        return;
    }
    duplicate_control(window);
    duplicate_control(window);
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    if ready
        .send((window as isize, unsafe { GetCurrentThreadId() }))
        .is_err()
    {
        unsafe { DestroyWindow(window) };
        return;
    }
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe { TranslateMessage(&message) };
        unsafe { DispatchMessageW(&message) };
    }
    unsafe { DestroyWindow(window) };
}

fn duplicate_control(parent: HWND) {
    let class = wide("BUTTON");
    let text = wide(NAME_MARKER);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | CONTROL_BORDER,
            CONTROL_LEFT,
            CONTROL_TOP,
            CONTROL_WIDTH,
            CONTROL_HEIGHT,
            parent,
            SHARED_CONTROL_ID as *mut c_void,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
}

/// Blocks until the window resolves to a UI Automation root, so a loaded
/// runner's first `WM_GETOBJECT` timing out is a wait rather than a failure.
fn await_walkable(handle: isize) -> Result<(), String> {
    let expiry = Instant::now() + WALKABLE_TIMEOUT;
    let mut last = String::from("the duplicate-pair window never resolved");
    while Instant::now() < expiry {
        let deadline =
            agent_desktop_core::Deadline::standard().map_err(|error| error.message.clone())?;
        match crate::tree::automation::root_from_hwnd(handle, deadline) {
            Ok(_) => return Ok(()),
            Err(error) => last = error.message.clone(),
        }
        std::thread::sleep(WALKABLE_POLL);
    }
    Err(last)
}

fn collect_marked(
    source: &UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &WalkBudget,
    out: &mut Vec<LocatorEvidence>,
) {
    if depth >= 8 {
        return;
    }
    let (_, evidence, _) = source.evidence(element);
    if evidence
        .name
        .known()
        .is_some_and(|name| name == NAME_MARKER)
    {
        out.push(evidence);
    }
    let mut ignored = false;
    let Ok(children) =
        crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)
    else {
        return;
    };
    for child in children {
        collect_marked(source, &child, depth + 1, budget, out);
    }
}

fn automation_id(evidence: &LocatorEvidence) -> Option<ElementIdentifier> {
    evidence
        .identifiers
        .identifiers()
        .iter()
        .find(|identifier| matches!(identifier.kind, IdentifierKind::AutomationId))
        .cloned()
}

/// Fails if any marker survived into any operator-visible slot of the error.
fn assert_redacted(error: &AdapterError, secret: &str, slot: &str) {
    let rendered = format!(
        "{} | {} | {} | {}",
        error.message,
        error.suggestion.clone().unwrap_or_default(),
        error.platform_detail.clone().unwrap_or_default(),
        error
            .details
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
    );
    assert!(
        !rendered.to_lowercase().contains(&secret.to_lowercase()),
        "the {slot} marker reached an ambiguity error: {rendered}"
    );
}

/// The `AMBIGUOUS_TARGET` branch, driven end to end against two live
/// controls no tier can separate, with the redaction commitment asserted
/// rather than eyeballed.
///
/// The pair shares role, name, `AutomationId` and rectangle, so the identity
/// tiers both match and the bounds tie-break is handed two equal hashes plus
/// the stored one - the arm that must stay ambiguous rather than pick a
/// candidate, since picking one is the silent-wrong-target shape A7-3
/// measured. The plan's own note is what the second half pins: macOS embeds
/// entry name, description and window title in its ambiguous details, and
/// Windows deliberately does not, so the resolver's answer carries shape
/// only - kind and candidate count - with no application-derived text in the
/// message, the suggestion, the platform detail or the details.
#[test]
fn duplicate_evidence_candidates_settle_ambiguous_target_carrying_no_application_text() {
    crate::tree::fixture::ensure_test_apartment();
    let hosted = DuplicatePairWindow::create().expect("a duplicate-evidence window hosts");
    let deadline = crate::tree::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(hosted.handle, deadline)
        .expect("the hosted window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut duplicates = Vec::new();
    collect_marked(&source, &prepared, 0, &budget, &mut duplicates);
    assert_eq!(
        duplicates.len(),
        2,
        "the host must present exactly two marked controls"
    );
    let (first, second) = (&duplicates[0], &duplicates[1]);
    let role = first.role.known().cloned().expect("a read role");
    let name = first.name.known().cloned().expect("a read name");
    let native_id = automation_id(first);
    let hash = crate::tree::resolve_match::bounds_hash_of(first);
    assert_eq!(second.role.known(), Some(&role), "the roles are equal");
    assert_eq!(second.name.known(), Some(&name), "the names are equal");
    assert_eq!(
        automation_id(second),
        native_id,
        "the identifiers are equal"
    );
    assert_eq!(
        crate::tree::resolve_match::bounds_hash_of(second),
        hash,
        "the bounds hashes are equal, so the tie-break cannot separate them"
    );
    let hash = hash.expect("a positive-area bounds hash");
    let rect = *first
        .ref_evidence
        .bounds
        .known()
        .expect("a read bounds rectangle");

    let pid = agent_desktop_core::ProcessId::from(std::process::id());
    let entry = RefEntry {
        process: agent_desktop_core::RefProcess {
            pid,
            process_instance: Some(
                crate::system::process_identity::token_for_pid(pid)
                    .expect("the token read answers")
                    .expect("a live process has a token"),
            ),
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role,
            name: Some(name),
            value: None,
            description: None,
            native_id,
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds: Some(rect),
            bounds_hash: Some(hash),
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: agent_desktop_core::RefSource {
            source_app: Some(APP_MARKER.into()),
            source_window_id: Some(format!("w-{}", hosted.handle)),
            source_window_title: Some(TITLE_MARKER.into()),
            source_window_bounds_hash: None,
            source_surface: agent_desktop_core::SnapshotSurface::Window,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: agent_desktop_core::refs::RefPath::default(),
        },
    };

    for evidence in &duplicates {
        assert_eq!(
            crate::tree::resolve_match::candidate_outcome(&entry, evidence),
            CandidateOutcome::Matched,
            "both live controls satisfy the identity tiers"
        );
    }

    let error = resolve_element_strict(&entry, deadline)
        .err()
        .expect("two indistinguishable candidates must not resolve to either one");

    assert_eq!(
        error.code,
        ErrorCode::AmbiguousTarget,
        "the resolver settles ambiguity instead of guessing: {error:?}"
    );
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("candidate_count"))
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "the shape-only details report how many candidates matched"
    );
    assert_redacted(&error, NAME_MARKER, "control name");
    assert_redacted(&error, TITLE_MARKER, "window title");
    assert_redacted(&error, APP_MARKER, "application file name");
    assert_redacted(
        &error,
        &automation_id(first)
            .expect("the shared control id surfaces as an automation id")
            .value,
        "automation id",
    );
}
