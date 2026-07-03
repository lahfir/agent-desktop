use std::collections::HashMap;
use std::path::PathBuf;

/// Options accepted by `launch_app_with_options`. `attach` defaults to
/// `true`, which preserves `launch_app`'s historical behavior of attaching
/// to an already-running instance instead of failing. Set it to `false` to
/// require a fresh launch: the adapter then fails with a structured error
/// naming the running pid instead of attaching.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub attach: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            attach: true,
        }
    }
}

#[cfg(test)]
#[path = "launch_options_tests.rs"]
mod tests;
