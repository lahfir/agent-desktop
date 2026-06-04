use agent_desktop_core::{
    adapter::PlatformAdapter,
    commands::{click, helpers::RefArgs, wait},
    context::CommandContext,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
};
use std::sync::{Mutex, MutexGuard};

static HOME_LOCK: Mutex<()> = Mutex::new(());

pub fn run_click_command(
    adapter: &dyn PlatformAdapter,
    entry: RefEntry,
) -> Result<serde_json::Value, agent_desktop_core::AppError> {
    let context = CommandContext::default();
    with_saved_entry(entry, &context, |snapshot_id| {
        click::execute(
            RefArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
            },
            adapter,
            &context,
        )
    })
}

pub fn run_wait_element_command(
    adapter: &dyn PlatformAdapter,
    entry: RefEntry,
    context: &CommandContext,
) -> Result<serde_json::Value, agent_desktop_core::AppError> {
    with_saved_entry(entry, context, |snapshot_id| {
        wait::execute_with_context(
            wait::WaitArgs {
                mode: wait::WaitModeArgs {
                    ms: None,
                    element: Some("@e1".into()),
                    window: None,
                    text: None,
                    menu: false,
                    menu_closed: false,
                    notification: false,
                },
                predicate: wait::WaitPredicateArgs {
                    snapshot_id: Some(snapshot_id),
                    predicate: Some("exists".into()),
                    value: None,
                    count: None,
                },
                timeout_ms: 100,
                app: None,
            },
            adapter,
            context,
        )
    })
}

fn with_saved_entry<T>(
    entry: RefEntry,
    context: &CommandContext,
    run: impl FnOnce(String) -> Result<T, agent_desktop_core::AppError>,
) -> Result<T, agent_desktop_core::AppError> {
    let _home = TestHome::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry);
    let snapshot_id = RefStore::for_session(context.session_id())?.save_new_snapshot(&refmap)?;
    run(snapshot_id)
}

struct TestHome {
    _lock: MutexGuard<'static, ()>,
    dir: std::path::PathBuf,
    prev: Option<std::ffi::OsString>,
}

impl TestHome {
    fn new() -> Self {
        let lock = HOME_LOCK.lock().unwrap();
        let dir = temp_path("home");
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &dir) };
        Self {
            _lock: lock,
            dir,
            prev,
        }
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        match self.prev.as_ref() {
            Some(prev) => unsafe { std::env::set_var("HOME", prev) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn temp_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "agent-desktop-conformance-{label}-{}",
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
