/// Integration tests for the snapshot command.
///
/// These tests require macOS with Accessibility permissions granted to the
/// terminal running the tests. They are skipped automatically on other
/// platforms or when the binary is not built.
#[cfg(test)]
mod tests {
    use std::process::Command;

    fn agent_desktop_bin() -> std::path::PathBuf {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        p.pop();
        p.push("agent-desktop");
        p
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_finder_returns_non_empty_tree() {
        let bin = agent_desktop_bin();
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "Finder"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
        assert!(json["data"]["ref_count"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_textedit_returns_refs() {
        let bin = agent_desktop_bin();
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "TextEdit"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
    }

    #[test]
    fn version_command_outputs_json() {
        let bin = agent_desktop_bin();
        if !bin.exists() {
            return; // binary not built yet
        }
        let output = Command::new(&bin)
            .args(["version", "--json"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
        assert!(json["data"]["version"].is_string());
    }

    #[test]
    fn list_apps_on_non_macos_errors_gracefully() {
        #[cfg(not(target_os = "macos"))]
        {
            let bin = agent_desktop_bin();
            if !bin.exists() {
                return;
            }
            let output = Command::new(&bin)
                .args(["list-apps"])
                .output()
                .expect("failed to run agent-desktop");

            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: serde_json::Value =
                serde_json::from_str(&stdout).expect("output is not valid JSON");

            assert_eq!(json["ok"], false);
            assert_eq!(json["error"]["code"], "PLATFORM_NOT_SUPPORTED");
        }
    }
}
