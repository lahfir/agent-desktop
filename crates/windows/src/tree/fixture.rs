use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use super::fixture_window;

pub(crate) use super::fixture_window::{CONTENT_MARKER, SECURE_MARKER};

const HOST_ENVIRONMENT_FLAG: &str = "AGENT_DESKTOP_FIXTURE_HOST";
const HOST_TEST_NAME: &str = "tree::fixture::tests::fixture_host_process_entry";
const HANDLE_PREFIX: &str = "AGENT_DESKTOP_FIXTURE_HWND=";
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const HOST_WATCHDOG_LIFETIME: Duration = Duration::from_secs(300);

/// Joins this thread to the multithreaded apartment for a test.
///
/// `ensure_owned_process_mta_and_dpi` is wrong here and the reason is not
/// cosmetic: `CoInitializeEx` is thread-local while that bootstrap's guard is
/// process-wide, so in a multi-threaded test binary only the first thread to
/// call it ever joins the apartment and every other thread sees
/// `CO_E_NOTINITIALIZED` from `CoCreateInstance`. Its own contract says as
/// much - it is sound for the CLI because the CLI calls it once from `main`
/// before any COM work. Libtest's worker threads are threads this product
/// does not own, which is exactly the case `CoIncrementMTAUsage` exists for.
pub(crate) fn ensure_test_apartment() {
    crate::system::com_runtime::ensure_hosted_library_mta_and_dpi()
        .expect("the process-wide MTA registration succeeds");
}

/// Reports whether this process was re-executed to host a fixture window.
pub(crate) fn is_host_process() -> bool {
    std::env::var(HOST_ENVIRONMENT_FLAG).is_ok()
}

/// Creates the window and pumps until the parent ends this process.
///
/// Only the re-executed child calls it. The watchdog bounds the process even
/// if the parent is killed without running `HostedFixture`'s `Drop`, so a
/// crashed test run cannot leave a window host behind on a shared runner.
pub(crate) fn run_as_host() {
    let (sender, receiver) = channel();
    spawn(move || {
        if let Ok(Ok(handle)) = receiver.recv_timeout(READY_TIMEOUT) {
            println!("{HANDLE_PREFIX}{handle}");
        }
    });
    spawn(|| {
        std::thread::sleep(HOST_WATCHDOG_LIFETIME);
        std::process::exit(0);
    });
    fixture_window::host_window(&fixture_window::unique_class_name(), sender);
}

/// A fixture window hosted in a second process.
///
/// This is the default for walk and cache tests. A window the test process
/// creates itself is served by in-process client-side providers, so the
/// failure taxonomy the walker exists to classify - RPC failure against
/// exhaustion, a target that stops pumping - is structurally unreachable
/// from it, and a cache policy validated there is validated against exactly
/// the provider class the policy says to skip.
pub(crate) struct HostedFixture {
    child: Option<Child>,
    handle: isize,
}

impl HostedFixture {
    pub(crate) fn spawn() -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut child = Command::new(executable)
            .args(["--exact", HOST_TEST_NAME, "--ignored", "--nocapture"])
            .env(HOST_ENVIRONMENT_FLAG, "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| String::from("the fixture host exposed no stdout"))?;
        let (sender, receiver) = channel();
        spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(handle) = line.trim().strip_prefix(HANDLE_PREFIX) {
                    let _ = sender.send(handle.trim().parse::<isize>().unwrap_or(0));
                    return;
                }
            }
            let _ = sender.send(0);
        });
        match receiver.recv_timeout(READY_TIMEOUT) {
            Ok(handle) if handle != 0 => Ok(Self {
                child: Some(child),
                handle,
            }),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                Err(String::from(
                    "the fixture host never reported a window handle",
                ))
            }
        }
    }

    pub(crate) fn handle(&self) -> isize {
        self.handle
    }

    pub(crate) fn process_id(&self) -> u32 {
        self.child.as_ref().map(Child::id).unwrap_or_default()
    }

    /// Ends the host process and waits for it, so a test can observe what the
    /// walk does against a provider that has genuinely gone away.
    pub(crate) fn terminate(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for HostedFixture {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// A fixture window hosted on a worker thread of the test process.
///
/// Retained only for teardown and concurrency tests. Never use it to validate
/// the walk's failure classification or the cache policy.
pub(crate) struct LocalFixture {
    handle: isize,
    class_name: String,
    pump: Option<JoinHandle<()>>,
}

impl LocalFixture {
    pub(crate) fn create() -> Result<Self, String> {
        let class_name = fixture_window::unique_class_name();
        let (sender, receiver) = channel();
        let pump = spawn({
            let class_name = class_name.clone();
            move || host_on_this_thread(&class_name, sender)
        });
        match receiver.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(handle)) => Ok(Self {
                handle,
                class_name,
                pump: Some(pump),
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(String::from("the fixture window never became ready")),
        }
    }

    pub(crate) fn handle(&self) -> isize {
        self.handle
    }

    pub(crate) fn geometry(&self) -> fixture_window::WindowGeometry {
        fixture_window::geometry(self.handle)
    }

    /// Identifies the thread that owns the window and runs its pump, so a
    /// test can assert that no UI Automation call is issued from it.
    pub(crate) fn pump_thread_id(&self) -> Option<std::thread::ThreadId> {
        self.pump.as_ref().map(|pump| pump.thread().id())
    }

    pub(crate) fn minimize(&self) {
        fixture_window::minimize_window(self.handle);
    }
}

impl Drop for LocalFixture {
    fn drop(&mut self) {
        fixture_window::close_window(self.handle);
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
        fixture_window::destroy_window(self.handle);
        fixture_window::unregister_class(&self.class_name);
    }
}

fn host_on_this_thread(class_name: &str, ready: Sender<Result<isize, String>>) {
    fixture_window::host_window(class_name, ready);
}

#[cfg(test)]
#[path = "fixture_tests.rs"]
mod tests;
