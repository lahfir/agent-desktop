use super::{
    clear, get_clipboard_content, reset_sequence_retries_observed, sequence_retries_observed,
    set_content,
};
use crate::input::clipboard_guard::MoveableMemory;
use crate::input::clipboard_session::ClipboardSession;
use crate::input::clipboard_text::encode_utf16_text;
use crate::system::png_codec::encode_bgra_to_png;
use crate::tree::fixture::bootstrap;
use crate::tree::fixture_clipboard::{
    ContendingClipboardHolder, DelayedClipboardOwner, clipboard_test_lock,
};
use agent_desktop_core::{
    AdapterError, ClipboardContent, ClipboardFormat, Deadline, DeliverySemantics, ErrorCode,
    ImageBuffer, ImageFormat, parse_png_dimensions,
};
use std::time::{Duration, Instant};

struct SavedClipboard {
    text: Option<String>,
    image: Option<Vec<u8>>,
    files: Option<Vec<String>>,
}

impl SavedClipboard {
    fn capture(deadline: Deadline) -> Self {
        Self {
            text: match get_clipboard_content(ClipboardFormat::Text, deadline) {
                Ok(Some(ClipboardContent::Text(value))) => Some(value),
                _ => None,
            },
            image: match get_clipboard_content(ClipboardFormat::Image, deadline) {
                Ok(Some(ClipboardContent::Image(image))) => Some(image.data),
                _ => None,
            },
            files: match get_clipboard_content(ClipboardFormat::FileUrls, deadline) {
                Ok(Some(ClipboardContent::FileUrls(paths))) => Some(paths),
                _ => None,
            },
        }
    }

    fn restore(&self, deadline: Deadline) {
        let _ = clear(deadline);
        if let Some(files) = &self.files {
            let _ = set_content(&ClipboardContent::FileUrls(files.clone()), deadline);
            return;
        }
        if let Some(png) = &self.image {
            if let Some((width, height)) = parse_png_dimensions(png) {
                let _ = set_content(
                    &ClipboardContent::Image(ImageBuffer {
                        data: png.clone(),
                        format: ImageFormat::Png,
                        width,
                        height,
                        scale_factor: 1.0,
                    }),
                    deadline,
                );
                return;
            }
        }
        if let Some(text) = &self.text {
            let _ = set_content(&ClipboardContent::Text(text.clone()), deadline);
        }
    }

    fn matches_current(&self, deadline: Deadline) -> bool {
        let current = Self::capture(deadline);
        current.text == self.text && current.image == self.image && current.files == self.files
    }
}

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn with_restored_clipboard(body: impl FnOnce()) {
    let _lock = clipboard_test_lock();
    bootstrap();
    let saved = SavedClipboard::capture(deadline());
    let body_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    saved.restore(deadline());
    if let Err(panic) = body_result {
        std::panic::resume_unwind(panic);
    }
    assert!(
        saved.matches_current(deadline()),
        "clipboard text/image/file-list content must be restored after the test"
    );
}

#[test]
fn text_image_and_files_round_trip_under_save_restore_lock() {
    with_restored_clipboard(|| {
        set_content(
            &ClipboardContent::Text("agent-desktop-u8".into()),
            deadline(),
        )
        .expect("set text");
        match get_clipboard_content(ClipboardFormat::Text, deadline())
            .expect("get text")
            .expect("text present")
        {
            ClipboardContent::Text(value) => assert_eq!(value, "agent-desktop-u8"),
            other => panic!("expected text, got {other:?}"),
        }

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
        .expect("set image");
        match get_clipboard_content(ClipboardFormat::Image, deadline())
            .expect("get image")
            .expect("image present")
        {
            ClipboardContent::Image(buffer) => {
                assert_eq!((buffer.width, buffer.height), (width, height));
                assert!(matches!(buffer.format, ImageFormat::Png));
                assert!(!buffer.data.is_empty());
            }
            other => panic!("expected image, got {other:?}"),
        }

        let path = std::env::temp_dir()
            .join("agent-desktop-clipboard-u8.txt")
            .to_string_lossy()
            .into_owned();
        set_content(&ClipboardContent::FileUrls(vec![path.clone()]), deadline())
            .expect("set files");
        match get_clipboard_content(ClipboardFormat::FileUrls, deadline())
            .expect("get files")
            .expect("files present")
        {
            ClipboardContent::FileUrls(paths) => assert_eq!(paths, vec![path]),
            other => panic!("expected files, got {other:?}"),
        }
    });
}

#[test]
fn empty_clipboard_returns_ok_none_distinct_from_transport_error() {
    with_restored_clipboard(|| {
        clear(deadline()).expect("clear");
        for format in [
            ClipboardFormat::Text,
            ClipboardFormat::Image,
            ClipboardFormat::FileUrls,
            ClipboardFormat::Auto,
        ] {
            assert!(
                get_clipboard_content(format, deadline())
                    .expect("absence is Ok")
                    .is_none()
            );
        }
        set_content(&ClipboardContent::Text("held".into()), deadline()).expect("seed");
        let mut holder = ContendingClipboardHolder::spawn().expect("holder");
        let error =
            get_clipboard_content(ClipboardFormat::Text, Deadline::after(200).expect("short"))
                .expect_err("contention against an advertised format is Err");
        assert_eq!(error.code, ErrorCode::Timeout);
        holder.release().expect("release");
    });
}

#[test]
fn auto_prefers_files_then_image_then_text() {
    with_restored_clipboard(|| {
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
        .expect("image");
        add_text_without_clearing().expect("add text");
        let auto = get_clipboard_content(ClipboardFormat::Auto, deadline())
            .expect("auto")
            .expect("content");
        assert!(matches!(auto, ClipboardContent::Image(_)));

        let path = std::env::temp_dir()
            .join("agent-desktop-clipboard-auto.txt")
            .to_string_lossy()
            .into_owned();
        set_content(&ClipboardContent::FileUrls(vec![path]), deadline()).expect("files");
        let auto = get_clipboard_content(ClipboardFormat::Auto, deadline())
            .expect("auto files")
            .expect("content");
        assert!(matches!(auto, ClipboardContent::FileUrls(_)));
    });
}

#[test]
fn hung_delay_owner_returns_app_unresponsive() {
    with_restored_clipboard(|| {
        let owner = DelayedClipboardOwner::create().expect("delayed owner");
        assert!(owner.format_available());
        let started = Instant::now();
        let error = get_clipboard_content(ClipboardFormat::Text, deadline())
            .expect_err("hung owner must fail closed");
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "pre-probe must return rather than hang on GetClipboardData"
        );
    });
}

#[test]
fn sequence_retry_is_observed_when_clipboard_moves_mid_read() {
    with_restored_clipboard(|| {
        reset_sequence_retries_observed();
        set_content(&ClipboardContent::Text("stable".into()), deadline()).expect("set");
        super::inject_sequence_mismatch_once();
        let content = get_clipboard_content(ClipboardFormat::Text, deadline())
            .expect("retry must still return content")
            .expect("text present");
        assert!(matches!(content, ClipboardContent::Text(_)));
        assert!(
            sequence_retries_observed() > 0,
            "a mid-read sequence move must trigger the stable-read retry"
        );
    });
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

fn add_text_without_clearing() -> Result<(), AdapterError> {
    let _session = ClipboardSession::open_for_write(deadline())?;
    let bytes = encode_utf16_text("alongside")?;
    MoveableMemory::from_bytes(&bytes)?.set_clipboard_data(13)?;
    Ok(())
}
