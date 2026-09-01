use super::{boolean_of, control_type_of};
use serde_json::{Value, json};
use uiautomation::types::UIProperty;
use uiautomation::UIElement;

/// Question 10's evidence: the two never-exercised ref-able arms.
///
/// `cell` (DataItem+TableItem/GridItem) and `switch` (Button+Toggle) are the
/// two role arms 2.3's dogfood could not observe. The fixture extension exists
/// so they can be, and this census records whether the extension actually
/// advertises the patterns: a `Button`-typed element with
/// `IsTogglePatternAvailable` true, and a `DataItem`-typed element with
/// `IsTableItemPatternAvailable` or `IsGridItemPatternAvailable` true.
pub fn measure_pattern_arms(elements: &[UIElement]) -> Value {
    let mut toggle_buttons = 0usize;
    let mut grid_item_cells = 0usize;
    let mut table_item_cells = 0usize;
    let mut grid_or_table_items = 0usize;
    let mut carrying_control_types: Vec<i32> = Vec::new();
    for element in elements {
        let control_type = control_type_of(element);
        let toggle_available = element
            .get_property_value(UIProperty::IsTogglePatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        let grid_available = element
            .get_property_value(UIProperty::IsGridItemPatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        let table_available = element
            .get_property_value(UIProperty::IsTableItemPatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        if control_type == 50000 && toggle_available {
            toggle_buttons += 1;
        }
        if grid_available {
            grid_item_cells += 1;
        }
        if table_available {
            table_item_cells += 1;
        }
        if grid_available || table_available {
            if !carrying_control_types.contains(&control_type) {
                carrying_control_types.push(control_type);
            }
            if control_type != 50029 {
                grid_or_table_items += 1;
            }
        }
    }
    carrying_control_types.sort_unstable();
    json!({
        "total_elements": elements.len(),
        "button_with_toggle": toggle_buttons,
        "any_element_with_griditem": grid_item_cells,
        "any_element_with_tableitem": table_item_cells,
        "non_dataitem_carrying_grid_or_tableitem": grid_or_table_items,
        "control_types_carrying_grid_or_tableitem": carrying_control_types,
    })
}

/// Reads the process-generation token of the window's owning process: the same
/// creation-time-derived identity the adapter's resolution gate verifies
/// against. Returns shape only - whether a token could be read at all is the
/// split-integrity evidence.
pub fn process_token_of_window(_handle: isize) -> Value {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(_handle as *mut core::ffi::c_void, &mut pid);
        }
        if pid == 0 {
            return json!({ "pid_zero": true });
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return json!({ "open_failed": true });
        }
        let mut created = windows_sys::Win32::Foundation::FILETIME::default();
        let mut exit = windows_sys::Win32::Foundation::FILETIME::default();
        let mut kernel = windows_sys::Win32::Foundation::FILETIME::default();
        let mut user = windows_sys::Win32::Foundation::FILETIME::default();
        unsafe {
            GetProcessTimes(
                process,
                &mut created,
                &mut exit,
                &mut kernel,
                &mut user,
            );
            CloseHandle(process);
        }
        let created_ticks = ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64;
        if created_ticks == 0 {
            return json!({ "creation_time_unavailable": true });
        }
        let seconds = created_ticks / 10_000_000;
        let micros = created_ticks % 10_000_000;
        json!({
            "creation_time_seconds": seconds,
            "creation_time_subtick": micros,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        json!({ "skipped": true })
    }
}
