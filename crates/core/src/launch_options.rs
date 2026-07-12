use std::collections::BTreeMap;
use std::path::PathBuf;

/// Options accepted by `launch_app`. `attach_if_running` defaults to `true`,
/// which preserves the behavior of attaching to an already-running instance
/// instead of failing. Set it to `false` to
/// require a fresh launch: the adapter then fails with a structured error
/// naming the running pid instead of attaching.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: u64,
    pub attach_if_running: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            timeout_ms: 5_000,
            attach_if_running: true,
        }
    }
}

#[cfg(test)]
#[path = "launch_options_tests.rs"]
mod tests;
