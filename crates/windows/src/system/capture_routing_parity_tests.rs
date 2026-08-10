use crate::adapter::WindowsAdapter;
use crate::input::clipboard::{clear, get_clipboard_content, set_content};
use crate::system::png_codec::encode_bgra_to_png;
use crate::system::private_file::WindowsPrivateFile;
use crate::tree::fixture::bootstrap;
use crate::tree::fixture_clipboard::clipboard_test_lock;
use agent_desktop_core::commands::clipboard_get::{self, ClipboardGetArgs};
use agent_desktop_core::commands::screenshot::{self, ScreenshotArgs};
use agent_desktop_core::{
    ClipboardContent, ClipboardFormat, CommandContext, Deadline, ImageBuffer, ImageFormat,
    PrivateFileOps, parse_png_dimensions,
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

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
            "agent-desktop-routing-home-{}-{}",
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

    fn path(&self) -> &Path {
        &self.root
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
    Deadline::after(10_000).expect("routing tests use a generous deadline")
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

fn create_junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("mklink /J must spawn");
    assert!(status.success(), "junction creation must succeed");
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
fn clipboard_image_default_path_travels_private_seam_reparse_and_owner() {
    with_restored_clipboard(|| {
        install_windows_private_file();
        let adapter = WindowsAdapter::new();
        let home = HomeIsolation::enter();
        let agent = home.path().join(".agent-desktop");
        let elsewhere = home.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("elsewhere");
        std::fs::create_dir_all(&agent).expect("agent-desktop");
        let tmp_junction = agent.join("tmp");
        create_junction(&tmp_junction, &elsewhere);

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
        .expect("seed");

        let refused = clipboard_get::execute(
            ClipboardGetArgs {
                format: Some(ClipboardFormat::Image),
                out: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect_err("reparse tmp must refuse private write");
        assert!(
            elsewhere
                .read_dir()
                .map(|entries| entries.count() == 0)
                .unwrap_or(true),
            "private refusal must leave the junction target empty"
        );
        let _ = refused;

        std::fs::remove_dir(&tmp_junction).expect("remove junction");
        std::fs::create_dir_all(&tmp_junction).expect("real tmp");
        let image = clipboard_get::execute(
            ClipboardGetArgs {
                format: Some(ClipboardFormat::Image),
                out: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect("reparse-free private write");
        let written = PathBuf::from(image["path"].as_str().expect("path"));
        assert!(written.starts_with(&tmp_junction));
        WindowsPrivateFile::new()
            .read_private_bounded(&written, 1024 * 1024)
            .expect("TokenOwner validation accepts the private artifact");
        let _ = std::fs::remove_file(&written);
    });
}

#[test]
fn screenshot_user_path_bypasses_private_policy_that_refuses_reparse() {
    bootstrap();
    install_windows_private_file();
    let adapter = WindowsAdapter::new();
    let root = std::env::temp_dir().join(format!(
        "agent-desktop-user-bypass-{}",
        std::process::id()
    ));
    let elsewhere = root.join("elsewhere");
    let junction = root.join("redirect");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");
    std::fs::create_dir_all(&root).expect("root");
    create_junction(&junction, &elsewhere);
    let user_path = junction.join("shot.png");

    let refused = WindowsPrivateFile::new().write_atomic(&user_path, b"private");
    assert!(
        refused.is_err(),
        "private policy must refuse the reparse destination"
    );
    assert!(!elsewhere.join("shot.png").exists());

    let response = screenshot::execute(
        ScreenshotArgs {
            app: None,
            window_id: None,
            screen: None,
            output_path: Some(user_path.clone()),
        },
        &adapter,
    )
    .expect("user-named PATH must bypass the private seam");
    assert_eq!(response["path"], user_path.to_string_lossy().as_ref());
    assert!(
        elsewhere.join("shot.png").is_file(),
        "user write must land through the junction"
    );

    let _ = std::fs::remove_file(elsewhere.join("shot.png"));
    let _ = std::fs::remove_dir(&junction);
    let _ = std::fs::remove_dir_all(&root);
}
