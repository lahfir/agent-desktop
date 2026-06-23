use agent_desktop_core::node::AppInfo;
use std::time::Duration;

const PS_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn list_apps() -> Vec<AppInfo> {
    let mut command = std::process::Command::new("/bin/ps");
    command.args(["-axo", "pid=,comm="]);
    let output = match crate::system::process::run_with_timeout(&mut command, "ps", PS_TIMEOUT) {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            tracing::debug!(status = ?output.status, "system: ps app inventory failed");
            return Vec::new();
        }
        Err(error) => {
            tracing::debug!(message = %error.message, "system: ps app inventory failed");
            return Vec::new();
        }
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen_pids = rustc_hash::FxHashSet::default();
    let mut apps = Vec::new();

    for line in text.lines() {
        let line = line.trim_start();
        let mut fields = line.splitn(2, char::is_whitespace);
        let Some(pid_text) = fields.next() else {
            continue;
        };
        let Some(command) = fields.next().map(str::trim) else {
            continue;
        };
        let Ok(pid) = pid_text.parse::<i32>() else {
            continue;
        };
        let Some(name) = app_name_from_command(command) else {
            continue;
        };
        if seen_pids.insert(pid) {
            apps.push(AppInfo {
                name,
                pid,
                bundle_id: None,
            });
        }
    }

    apps
}

fn app_name_from_command(command: &str) -> Option<String> {
    if command.contains("/Contents/Frameworks/")
        || command.contains("/Contents/PlugIns/")
        || command.contains("/XPCServices/")
        || command.contains(".appex/")
    {
        return None;
    }

    let marker = ".app/Contents/MacOS";
    let marker_start = command.find(marker)?;
    let app_path = &command[..marker_start + ".app".len()];
    let app_name = app_path.rsplit('/').next()?.strip_suffix(".app")?;
    if app_name.is_empty() {
        None
    } else {
        Some(app_name.to_string())
    }
}

#[cfg(test)]
#[path = "process_apps_tests.rs"]
mod tests;
