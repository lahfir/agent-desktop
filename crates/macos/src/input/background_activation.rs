use crate::input::skylight;

pub(crate) const EVENT_RECORD_LEN: usize = 0xF8;

/// Builds the 248-byte window-server event record that tells the target
/// process its window `window_number` gained focus.
///
/// Layout follows background-computer-use's `targetOnlyFocus` record (and the
/// yabai/cua focus records it derives from): byte `0x04` holds the record
/// length, `0x08` the event kind `0x0D`, `0x3C..0x40` the little-endian window
/// number, and `0x8A` the focus flag. Unlike those projects' full focus swap,
/// no matching defocus record is ever sent to the user's frontmost app, so
/// only the target's own idea of its focused window changes.
pub(crate) fn target_focus_record(window_number: u32) -> [u8; EVENT_RECORD_LEN] {
    let mut record = [0_u8; EVENT_RECORD_LEN];
    record[0x04] = 0xF8;
    record[0x08] = 0x0D;
    record[0x3C..0x40].copy_from_slice(&window_number.to_le_bytes());
    record[0x8A] = 0x01;
    record
}

/// Posts [`target_focus_record`] to the target process only. `Err` carries
/// the degradation reason reported to the caller; delivery continues without
/// activation in that case.
pub(crate) fn focus_target_window(pid: libc::pid_t, window_number: u32) -> Result<(), String> {
    let psn = skylight::process_serial_number(pid)
        .ok_or_else(|| "activate:GetProcessForPID_unavailable".to_string())?;
    match skylight::post_event_record(&psn, &target_focus_record(window_number)) {
        None => Err("activate:SLPSPostEventRecordTo_unavailable".to_string()),
        Some(0) => Ok(()),
        Some(status) => Err(format!("activate:SLPSPostEventRecordTo_failed_{status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_record_sets_only_the_documented_bytes() {
        let record = target_focus_record(0x0000_3B8F);
        assert_eq!(record.len(), 248);
        assert_eq!(record[0x04], 0xF8);
        assert_eq!(record[0x08], 0x0D);
        assert_eq!(&record[0x3C..0x40], &[0x8F, 0x3B, 0x00, 0x00]);
        assert_eq!(record[0x8A], 0x01);

        let set_bytes: Vec<usize> = record
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte != 0)
            .map(|(offset, _)| offset)
            .collect();
        assert_eq!(set_bytes, [0x04, 0x08, 0x3C, 0x3D, 0x8A]);
    }

    #[test]
    fn window_number_is_little_endian_across_all_four_bytes() {
        let record = target_focus_record(0x1234_5678);
        assert_eq!(&record[0x3C..0x40], &[0x78, 0x56, 0x34, 0x12]);
    }
}
