use agent_desktop_core::{
    adapter::{ImageBuffer, ImageFormat},
    error::{AdapterError, ErrorCode},
    image_buffer::parse_png_dimensions,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const SCREENCAPTURE: &str = "/usr/sbin/screencapture";
    #[cfg(not(test))]
    const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(5);
    #[cfg(test)]
    const SCREENSHOT_TIMEOUT: Duration = Duration::from_millis(20);
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn capture(
        window_id: Option<u32>,
        display_index: Option<usize>,
    ) -> Result<ImageBuffer, AdapterError> {
        let scale_factor = display_index
            .map(crate::system::display::display_at)
            .transpose()?
            .map(|display| display.scale)
            .unwrap_or_else(primary_display_scale);
        let temp = TempPng::new()?;
        let mut command = Command::new(SCREENCAPTURE);
        command.args(["-x", "-t", "png"]);

        if let Some(wid) = window_id {
            command.args(["-l", &wid.to_string()]);
        } else if let Some(idx) = display_index {
            let display = crate::system::display::display_at(idx)?;
            let x = (display.bounds.x * display.scale).round() as i32;
            let y = (display.bounds.y * display.scale).round() as i32;
            let w = (display.bounds.width * display.scale).round() as i32;
            let h = (display.bounds.height * display.scale).round() as i32;
            command.args(["-R", &format!("{x},{y},{w},{h}")]);
        }

        command.arg(temp.path());
        let output = run_screencapture(&mut command)?;

        if !output.status.success() {
            return Err(map_screencapture_error(&output));
        }

        let mut buffer = read_png(temp.path())?;
        buffer.scale_factor = scale_factor;
        Ok(buffer)
    }

    fn primary_display_scale() -> f64 {
        crate::system::display::display_at(0)
            .map(|display| display.scale)
            .unwrap_or(1.0)
    }

    pub fn capture_app(pid: i32) -> Result<ImageBuffer, AdapterError> {
        tracing::debug!("system: screenshot app pid={pid}");
        capture(find_cg_window_id_for_pid(pid), None)
    }

    pub fn capture_screen(idx: usize) -> Result<ImageBuffer, AdapterError> {
        tracing::debug!("system: screenshot screen idx={idx}");
        capture(None, Some(idx))
    }

    struct TempPng {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TempPng {
        fn new() -> Result<Self, AdapterError> {
            let mut dir = std::env::temp_dir();
            dir.push(format!("agent-desktop-screenshot-{}", unique_suffix()));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .map_err(|e| AdapterError::internal(format!("create screenshot temp dir: {e}")))?;
            let path = dir.join("capture.png");
            Ok(Self { dir, path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempPng {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.dir);
        }
    }

    fn unique_suffix() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{}-{nanos}-{seq}", std::process::id())
    }

    fn run_screencapture(command: &mut Command) -> Result<Output, AdapterError> {
        crate::system::process::run_with_timeout(command, "screencapture", SCREENSHOT_TIMEOUT)
    }

    fn read_png(path: &Path) -> Result<ImageBuffer, AdapterError> {
        let data = std::fs::read(path)
            .map_err(|e| AdapterError::internal(format!("read screenshot: {e}")))?;
        let (width, height) = parse_png_dimensions(&data).unwrap_or((0, 0));
        Ok(ImageBuffer {
            data,
            format: ImageFormat::Png,
            width,
            height,
            scale_factor: 1.0,
        })
    }

    fn map_screencapture_error(output: &Output) -> AdapterError {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stderr}\n{stdout}");
        let lower = combined.to_lowercase();
        if lower.contains("screen recording")
            || lower.contains("not authorized")
            || lower.contains("permission")
            || lower.contains("denied")
        {
            return AdapterError::new(ErrorCode::PermDenied, "Screen Recording permission denied")
                .with_suggestion(
                    "Open System Settings > Privacy & Security > Screen Recording and add the app that launches agent-desktop. If macOS lists the built binary separately, add that binary too.",
                )
                .with_platform_detail(combined.trim());
        }

        let detail = combined.trim();
        let detail = if detail.is_empty() {
            "screencapture produced no diagnostic output"
        } else {
            detail
        };
        AdapterError::internal("screencapture exited with error").with_platform_detail(detail)
    }

    fn find_cg_window_id_for_pid(pid: i32) -> Option<u32> {
        let mut best_id: Option<u32> = None;
        let mut best_area: f64 = 0.0;

        for record in crate::system::cg_window::visible_window_records() {
            if record.pid != pid {
                continue;
            }

            if record.area > best_area {
                best_area = record.area;
                best_id = Some(record.window_number as u32);
            }
        }

        best_id
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        fn output(stderr: &str) -> Output {
            Output {
                status: ExitStatus::from_raw(1 << 8),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            }
        }

        #[test]
        fn maps_screen_recording_error_to_permission_denied() {
            let err = map_screencapture_error(&output("Screen Recording is not authorized"));

            assert_eq!(err.code, ErrorCode::PermDenied);
            assert!(err.suggestion.is_some());
        }

        #[test]
        fn maps_unknown_screencapture_error_to_internal() {
            let err = map_screencapture_error(&output("display server unavailable"));

            assert_eq!(err.code, ErrorCode::Internal);
            assert_eq!(
                err.platform_detail.as_deref(),
                Some("display server unavailable")
            );
        }

        #[test]
        fn run_screencapture_kills_timed_out_process() {
            let mut command = Command::new("/bin/sleep");
            command.arg("10");

            let err = run_screencapture(&mut command).unwrap_err();

            assert_eq!(err.code, ErrorCode::Timeout);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn capture_app(_pid: i32) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_app"))
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_screen"))
    }
}

pub use imp::{capture_app, capture_screen};
