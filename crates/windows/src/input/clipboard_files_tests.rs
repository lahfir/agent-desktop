use super::{DROPFILES_SIZE, decode_hdrop, encode_hdrop};
use agent_desktop_core::ErrorCode;

fn utf16_le(units: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len() * 2);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

fn path_units(path: &str) -> Vec<u16> {
    path.encode_utf16().collect()
}

#[test]
fn hdrop_round_trips_empty_single_and_multiple() {
    let cases: &[(&[String], &str)] = &[
        (&[], "empty"),
        (&["C:\\only.txt".to_string()], "single"),
        (
            &[
                "C:\\a.txt".to_string(),
                "C:\\b dir\\c.txt".to_string(),
                "D:\\third".to_string(),
            ],
            "multiple",
        ),
    ];
    for (paths, label) in cases {
        let encoded = encode_hdrop(paths).unwrap_or_else(|error| {
            panic!("encode failed for {label}: {error:?}");
        });
        assert_eq!(
            &encoded[0..4],
            &(DROPFILES_SIZE as u32).to_le_bytes(),
            "{label} pFiles"
        );
        assert_eq!(encoded[16..20], 1i32.to_le_bytes(), "{label} fWide");
        let list = &encoded[DROPFILES_SIZE..];
        assert!(
            list.ends_with(&[0, 0, 0, 0]),
            "{label} must end with a wide double-NUL"
        );
        assert_eq!(
            list.len() % 2,
            0,
            "{label} wide list must be UTF-16 aligned"
        );
        let units: Vec<u16> = list
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let first_double_nul = units
            .windows(2)
            .position(|pair| pair == [0, 0])
            .expect("double-NUL present");
        assert_eq!(
            first_double_nul,
            units.len() - 2,
            "{label} double terminator must appear exactly once at the end"
        );
        let decoded = decode_hdrop(&encoded).unwrap_or_else(|error| {
            panic!("decode failed for {label}: {error:?}");
        });
        assert_eq!(&decoded, paths, "{label}");
    }
}

#[test]
fn empty_list_payload_decodes_to_empty_vector() {
    let mut payload = vec![0u8; DROPFILES_SIZE + 4];
    payload[0..4].copy_from_slice(&(DROPFILES_SIZE as u32).to_le_bytes());
    payload[16..20].copy_from_slice(&1i32.to_le_bytes());
    let decoded = decode_hdrop(&payload).expect("empty HDROP");
    assert!(decoded.is_empty());
}

#[test]
fn truncated_header_is_rejected() {
    let error = decode_hdrop(&[0u8; DROPFILES_SIZE - 1]).expect_err("short header");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn truncated_path_list_is_rejected() {
    let mut units = path_units("C:\\a.txt");
    units.push(0);
    let mut payload = vec![0u8; DROPFILES_SIZE];
    payload[0..4].copy_from_slice(&(DROPFILES_SIZE as u32).to_le_bytes());
    payload[16..20].copy_from_slice(&1i32.to_le_bytes());
    payload.extend(utf16_le(&units));
    let error = decode_hdrop(&payload).expect_err("missing final NUL");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn inconsistent_pfiles_offset_is_rejected() {
    let mut payload = encode_hdrop(&["C:\\a.txt".to_string()]).expect("valid payload");
    payload[0..4].copy_from_slice(&0u32.to_le_bytes());
    let error = decode_hdrop(&payload).expect_err("bad pFiles");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn ansi_hdrop_is_rejected() {
    let mut payload = encode_hdrop(&["C:\\a.txt".to_string()]).expect("valid payload");
    payload[16..20].copy_from_slice(&0i32.to_le_bytes());
    let error = decode_hdrop(&payload).expect_err("ANSI");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn embedded_nul_path_encode_is_rejected() {
    let error = encode_hdrop(&["C:\\a\0b.txt".to_string()]).expect_err("embedded NUL");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}
