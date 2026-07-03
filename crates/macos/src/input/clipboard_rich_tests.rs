use super::*;

fn fake_png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; 24];
    bytes[0..8].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    bytes
}

#[test]
fn png_dimensions_reads_ihdr_width_and_height() {
    let bytes = fake_png(640, 480);
    assert_eq!(png_dimensions(&bytes), (640, 480));
}

#[test]
fn png_dimensions_of_undersized_buffer_is_zero_not_panic() {
    assert_eq!(png_dimensions(&[1, 2, 3]), (0, 0));
}

#[test]
fn file_url_to_path_round_trips_a_plain_path() {
    let path = file_url_to_path("file:///tmp/agent-desktop-test.txt")
        .expect("well-formed file:// URL must decode to a path");
    assert_eq!(path, "/tmp/agent-desktop-test.txt");
}

#[test]
fn file_url_to_path_decodes_percent_escaped_spaces() {
    let path = file_url_to_path("file:///tmp/agent%20desktop/note.txt")
        .expect("percent-encoded file:// URL must decode");
    assert_eq!(path, "/tmp/agent desktop/note.txt");
}

#[test]
fn file_url_to_path_rejects_non_file_scheme() {
    assert!(file_url_to_path("https://example.com/a.txt").is_none());
}
