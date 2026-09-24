use crate::input::skylight;

/// Length the window server reads from a focus or key-window record.
pub(crate) const EVENT_RECORD_LEN: usize = 0xF8;

/// Every record is backed by a larger zeroed buffer because
/// `SLPSPostEventRecordTo` reads up to 8 bytes past the declared length on
/// macOS 14.2.1+ and 26, which crashed the process when the buffer ended
/// exactly at `EVENT_RECORD_LEN` (a896d9c5).
pub(crate) const EVENT_RECORD_BUFFER: usize = 0x100;

pub(crate) type EventRecord = [u8; EVENT_RECORD_BUFFER];

/// Builds the window-server event record that tells the target process its
/// window `window_number` gained focus.
///
/// Layout follows background-computer-use's `targetOnlyFocus` record (and the
/// yabai/cua focus records it derives from): byte `0x04` holds the record
/// length, `0x08` the event kind `0x0D`, `0x3C..0x40` the little-endian window
/// number, and `0x8A` the focus flag. Unlike those projects' full focus swap,
/// no matching defocus record is ever sent to the user's frontmost app, so
/// only the target's own idea of its focused window changes.
pub(crate) fn target_focus_record(window_number: u32) -> EventRecord {
    let mut record = [0_u8; EVENT_RECORD_BUFFER];
    record[0x04] = EVENT_RECORD_LEN as u8;
    record[0x08] = 0x0D;
    record[0x3C..0x40].copy_from_slice(&window_number.to_le_bytes());
    record[0x8A] = 0x01;
    record
}

/// Builds yabai's `window_manager_make_key_window` record pair, which makes
/// `window_number` the key window inside its own process: byte `0x04` holds
/// the record length, `0x08` the phase (`0x01` then `0x02`), `0x20..0x30` is
/// filled with `0xFF`, `0x3A` is `0x10`, and `0x3C..0x40` holds the
/// little-endian window number. yabai sends it after making the process
/// frontmost; here it goes to the target alone, so the target routes
/// incoming keys to this window while another app stays frontmost.
pub(crate) fn key_window_records(window_number: u32) -> [EventRecord; 2] {
    let mut template = [0_u8; EVENT_RECORD_BUFFER];
    template[0x04] = EVENT_RECORD_LEN as u8;
    template[0x20..0x30].fill(0xFF);
    template[0x3A] = 0x10;
    template[0x3C..0x40].copy_from_slice(&window_number.to_le_bytes());

    let mut first = template;
    let mut second = template;
    first[0x08] = 0x01;
    second[0x08] = 0x02;
    [first, second]
}

/// Posts [`target_focus_record`] to the target process only. `Err` carries
/// the degradation reason reported to the caller; delivery continues without
/// activation in that case.
pub(crate) fn focus_target_window(pid: libc::pid_t, window_number: u32) -> Result<(), String> {
    post_records(
        "activate",
        pid,
        std::slice::from_ref(&target_focus_record(window_number)),
    )
}

/// Posts [`key_window_records`] to the target process only, with the same
/// degradation contract as [`focus_target_window`].
pub(crate) fn make_key_window(pid: libc::pid_t, window_number: u32) -> Result<(), String> {
    post_records("keywindow", pid, &key_window_records(window_number))
}

fn post_records(layer: &str, pid: libc::pid_t, records: &[EventRecord]) -> Result<(), String> {
    let psn = skylight::process_serial_number(pid)
        .ok_or_else(|| format!("{layer}:GetProcessForPID_unavailable"))?;
    for record in records {
        match skylight::post_event_record(&psn, record) {
            None => return Err(format!("{layer}:SLPSPostEventRecordTo_unavailable")),
            Some(0) => {}
            Some(status) => return Err(format!("{layer}:SLPSPostEventRecordTo_failed_{status}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_bytes(record: &EventRecord) -> Vec<usize> {
        record
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte != 0)
            .map(|(offset, _)| offset)
            .collect()
    }

    #[test]
    fn focus_record_sets_only_the_documented_bytes() {
        let record = target_focus_record(0x0000_3B8F);
        assert_eq!(record.len(), 256, "padded past the 0xF8 the server reads");
        assert_eq!(record[0x04], 0xF8);
        assert_eq!(record[0x08], 0x0D);
        assert_eq!(&record[0x3C..0x40], &[0x8F, 0x3B, 0x00, 0x00]);
        assert_eq!(record[0x8A], 0x01);
        assert_eq!(set_bytes(&record), [0x04, 0x08, 0x3C, 0x3D, 0x8A]);
    }

    #[test]
    fn window_number_is_little_endian_across_all_four_bytes() {
        let record = target_focus_record(0x1234_5678);
        assert_eq!(&record[0x3C..0x40], &[0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn key_window_records_match_yabai_and_differ_only_in_phase() {
        let [first, second] = key_window_records(0x1234_5678);
        for record in [&first, &second] {
            assert_eq!(record.len(), 256);
            assert_eq!(record[0x04], 0xF8);
            assert!(record[0x20..0x30].iter().all(|byte| *byte == 0xFF));
            assert_eq!(record[0x3A], 0x10);
            assert_eq!(&record[0x3C..0x40], &[0x78, 0x56, 0x34, 0x12]);
            assert!(record[EVENT_RECORD_LEN..].iter().all(|byte| *byte == 0));
        }
        assert_eq!((first[0x08], second[0x08]), (0x01, 0x02));

        let differing: Vec<usize> = (0..EVENT_RECORD_BUFFER)
            .filter(|offset| first[*offset] != second[*offset])
            .collect();
        assert_eq!(differing, [0x08]);
    }
}
