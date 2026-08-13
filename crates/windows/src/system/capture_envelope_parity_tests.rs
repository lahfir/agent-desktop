use crate::adapter::WindowsAdapter;
use crate::input::clipboard::{clear, get_clipboard_content, set_content};
use crate::system::png_codec::encode_bgra_to_png;
use crate::system::private_file::WindowsPrivateFile;
use crate::tree::fixture::{LocalPatternFixture, bootstrap};
use crate::tree::fixture_clipboard::clipboard_test_lock;
use agent_desktop_core::commands::clipboard_clear;
use agent_desktop_core::commands::clipboard_get::{self, ClipboardGetArgs};
use agent_desktop_core::commands::clipboard_set::{self, ClipboardSetArgs};
use agent_desktop_core::commands::screenshot::{self, ScreenshotArgs};
use agent_desktop_core::{
    AppError, ClipboardContent, ClipboardFormat, CommandContext, Deadline, DeliverySemantics,
    ErrorCode, ErrorPayload, ImageBuffer, ImageFormat, PrivateFileOps, ProcessId, ScreenshotTarget,
    SystemOps, WindowInfo, WindowState, parse_png_dimensions,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

const LIVE_STAGE_VARIABLE: &str = "AGENT_DESKTOP_LIVE_WPF";

static HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

struct HomeIsolation {
    previous_home: Option<std::ffi::OsString>,
    previous_profile: Option<std::ffi::OsString>,
    root: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl HomeIsolation {
    fn enter() -> Self {
        let lock = HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "agent-desktop-capture-home-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&root).expect("isolated home");
        let previous_home = std::env::var_os("HOME");
        let previous_profile = std::env::var_os("USERPROFILE");
        unsafe {
            std::env::set_var("HOME", &root);
            std::env::set_var("USERPROFILE", &root);
        }
        Self {
            previous_home,
            previous_profile,
            root,
            _lock: lock,
        }
    }
}

impl Drop for HomeIsolation {
    fn drop(&mut self) {
        match &self.previous_home {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match &self.previous_profile {
            Some(value) => unsafe { std::env::set_var("USERPROFILE", value) },
            None => unsafe { std::env::remove_var("USERPROFILE") },
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("envelope tests use a generous deadline")
}

fn install_windows_private_file() {
    let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
}

fn sample_png() -> Vec<u8> {
    encode_bgra_to_png(
        &[
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ],
        2,
        2,
        8,
        deadline(),
    )
    .expect("png")
}

fn keys_of(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("data must be an object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn error_wire(error: &agent_desktop_core::AdapterError) -> Value {
    serde_json::to_value(ErrorPayload::from_app_error(&AppError::from(error.clone())))
        .expect("ErrorPayload serializes")
}

fn assert_disposition_wire(error: &agent_desktop_core::AdapterError, expected: DeliverySemantics) {
    let wire = error_wire(error);
    let projected = serde_json::to_value(expected).expect("disposition serializes");
    assert_eq!(wire["disposition"], projected, "disposition wire shape");
}

fn with_restored_clipboard(body: impl FnOnce()) {
    let _lock = clipboard_test_lock();
    bootstrap();
    let saved_text = match get_clipboard_content(ClipboardFormat::Text, deadline()) {
        Ok(Some(ClipboardContent::Text(value))) => Some(value),
        _ => None,
    };
    let body_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    let _ = clear(deadline());
    if let Some(text) = saved_text {
        let _ = set_content(&ClipboardContent::Text(text), deadline());
    }
    if let Err(panic) = body_result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn screenshot_failure_disposition_serializes_not_delivered() {
    bootstrap();
    let adapter = WindowsAdapter::new();
    let fixture = LocalPatternFixture::create().expect("pattern fixture");
    let info = WindowInfo {
        id: format!("w-{}", fixture.handle() as usize),
        title: String::new(),
        app: String::new(),
        pid: ProcessId::from(std::process::id()),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };
    let error = SystemOps::screenshot(&adapter, ScreenshotTarget::ExactWindow(info), deadline())
        .expect_err("ExactWindow without process_instance must fail before capture");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_disposition_wire(&error, DeliverySemantics::not_delivered());
}

#[test]
fn screenshot_path_and_inline_data_shapes_match_core_serialization() {
    bootstrap();
    install_windows_private_file();
    let adapter = WindowsAdapter::new();
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-screenshot-envelope-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch");
    let path = dir.join("shot.png");

    let with_path = screenshot::execute(
        ScreenshotArgs {
            app: None,
            window_id: None,
            screen: None,
            output_path: Some(path.clone()),
        },
        &adapter,
    )
    .expect("screenshot path");
    assert_eq!(
        keys_of(&with_path),
        ["format", "height", "path", "scale_factor", "width"]
    );
    assert_eq!(with_path["path"], path.to_string_lossy().as_ref());
    assert_eq!(with_path["format"], "png");
    assert!(with_path["width"].as_u64().unwrap() > 0);
    assert!(with_path["height"].as_u64().unwrap() > 0);
    assert!(with_path["scale_factor"].as_f64().unwrap() > 0.0);
    assert!(path.is_file(), "user-named PATH must land bytes");

    let inline = screenshot::execute(
        ScreenshotArgs {
            app: None,
            window_id: None,
            screen: None,
            output_path: None,
        },
        &adapter,
    )
    .expect("screenshot inline");
    assert_eq!(
        keys_of(&inline),
        ["data", "format", "height", "scale_factor", "width"]
    );
    assert!(inline["data"].as_str().unwrap().len() > 8);
    assert_eq!(inline["format"], "png");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!path.exists(), "screenshot scratch must be removed");
}

#[test]
fn clipboard_get_set_clear_data_shapes_match_core_serialization() {
    with_restored_clipboard(|| {
        install_windows_private_file();
        let adapter = WindowsAdapter::new();
        let _home = HomeIsolation::enter();

        clear(deadline()).expect("clear before found:false");
        let missing = clipboard_get::execute(
            ClipboardGetArgs {
                format: Some(ClipboardFormat::Image),
                out: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect("empty clipboard is structured absence");
        assert_eq!(keys_of(&missing), ["found", "type"]);
        assert_eq!(missing["type"], "image");
        assert_eq!(missing["found"], false);

        let set = clipboard_set::execute(
            ClipboardSetArgs {
                text: Some("envelope-parity-marker".into()),
                image: None,
                file_urls: vec![],
            },
            &adapter,
        )
        .expect("clipboard-set");
        assert_eq!(keys_of(&set), ["ok", "type"]);
        assert_eq!(set["ok"], true);
        assert_eq!(set["type"], "text");

        let got = clipboard_get::execute(
            ClipboardGetArgs {
                format: Some(ClipboardFormat::Text),
                out: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect("clipboard-get text");
        assert_eq!(keys_of(&got), ["text", "type"]);
        assert_eq!(got["type"], "text");
        assert_eq!(got["text"], "envelope-parity-marker");

        let png = sample_png();
        let (width, height) = parse_png_dimensions(&png).expect("dims");
        set_content(
            &ClipboardContent::Image(ImageBuffer {
                data: png,
                format: ImageFormat::Png,
                width,
                height,
                scale_factor: 1.0,
            }),
            deadline(),
        )
        .expect("seed image");
        let image = clipboard_get::execute(
            ClipboardGetArgs {
                format: Some(ClipboardFormat::Image),
                out: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect("clipboard-get image");
        assert_eq!(keys_of(&image), ["height", "path", "type", "width"]);
        assert_eq!(image["type"], "image");
        assert_eq!(image["width"], width);
        assert_eq!(image["height"], height);
        let written = PathBuf::from(image["path"].as_str().expect("path string"));
        assert!(written.is_file());
        let bytes = WindowsPrivateFile::new()
            .read_private_bounded(&written, 1024 * 1024)
            .expect("TokenOwner-owned private image must be readable");
        assert!(!bytes.is_empty());
        let _ = std::fs::remove_file(&written);

        let cleared = clipboard_clear::execute(&adapter).expect("clipboard-clear");
        assert_eq!(keys_of(&cleared), ["cleared"]);
        assert_eq!(cleared["cleared"], true);
    });
}

#[test]
fn non_one_scale_factor_capture_skipped_on_96dpi_only_hosts() {
    bootstrap();
    let displays = crate::system::display::list_displays_live(deadline()).expect("displays");
    let Some(scaled) = displays
        .iter()
        .find(|display| (display.scale - 1.0).abs() > 0.001)
    else {
        eprintln!(
            "skip: non-1.0 scale_factor capture needs a display above 96 DPI; every measured host so far is 96 DPI only (A10-3/A16-4), and a second display is not manufacturable here"
        );
        return;
    };
    let image = crate::system::screenshot::screenshot(
        agent_desktop_core::ScreenshotTarget::Display {
            index: displays
                .iter()
                .position(|display| display.id == scaled.id)
                .expect("scaled display index"),
            expected: scaled.clone(),
        },
        deadline(),
    )
    .expect("scaled display capture");
    assert!(
        (image.scale_factor - scaled.scale).abs() < 0.001,
        "capture must report the owning display scale {}, got {}",
        scaled.scale,
        image.scale_factor
    );
}

#[test]
fn the_windows_lane_opts_into_live_capture_staging() {
    let workflow = include_str!("../../../../.github/workflows/ci.yml").replace("\r\n", "\n");
    let assignment = format!("{LIVE_STAGE_VARIABLE}: \"1\"");
    let step = workflow
        .split("- name: ")
        .find(|step| step.starts_with("Core and Windows unit tests"))
        .expect("the Windows lane runs a library-test step");
    assert!(
        step.contains(&assignment),
        "the library-test step must opt into on-screen staging for live capture breadth"
    );
}
