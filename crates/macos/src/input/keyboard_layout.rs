#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LayoutLookup {
    Found(u16),
    NoKeyForSymbol,
    LayoutUnavailable,
}

pub(crate) fn code_for_symbol(symbol: char) -> LayoutLookup {
    match platform::with_translator(|translate| find_code(symbol, translate)) {
        Some(Some(code)) => LayoutLookup::Found(code),
        Some(None) => LayoutLookup::NoKeyForSymbol,
        None => LayoutLookup::LayoutUnavailable,
    }
}

pub(crate) fn find_code(symbol: char, translate: impl Fn(u16) -> Option<char>) -> Option<u16> {
    (0..128).find(|&code| translate(code) == Some(symbol))
}

#[cfg(target_os = "macos")]
mod platform {
    use core_foundation_sys::{
        base::CFRelease,
        data::{CFDataGetBytePtr, CFDataRef},
        string::CFStringRef,
    };
    use std::ffi::c_void;

    const KUC_KEY_ACTION_DOWN: u16 = 0;
    const KUC_KEY_TRANSLATE_NO_DEAD_KEYS: u32 = 1;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut c_void;
        fn TISGetInputSourceProperty(source: *mut c_void, key: CFStringRef) -> *const c_void;
        static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
        fn LMGetKbdType() -> u8;
        fn UCKeyTranslate(
            layout: *const c_void,
            virtual_key_code: u16,
            key_action: u16,
            modifier_key_state: u32,
            keyboard_type: u32,
            key_translate_options: u32,
            dead_key_state: *mut u32,
            max_string_length: usize,
            actual_string_length: *mut usize,
            unicode_string: *mut u16,
        ) -> i32;
    }

    pub(super) fn with_translator<T>(
        run: impl FnOnce(&dyn Fn(u16) -> Option<char>) -> T,
    ) -> Option<T> {
        unsafe {
            let source = TISCopyCurrentKeyboardLayoutInputSource();
            if source.is_null() {
                return None;
            }
            let data =
                TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) as CFDataRef;
            let result = (!data.is_null()).then(|| {
                let layout = CFDataGetBytePtr(data).cast::<c_void>();
                let keyboard_type = u32::from(LMGetKbdType());
                run(&|code| translate(layout, keyboard_type, code))
            });
            CFRelease(source.cast_const());
            result
        }
    }

    unsafe fn translate(layout: *const c_void, keyboard_type: u32, code: u16) -> Option<char> {
        let mut dead_key_state = 0;
        let mut length = 0;
        let mut buffer = [0_u16; 4];
        let status = unsafe {
            UCKeyTranslate(
                layout,
                code,
                KUC_KEY_ACTION_DOWN,
                0,
                keyboard_type,
                KUC_KEY_TRANSLATE_NO_DEAD_KEYS,
                &mut dead_key_state,
                buffer.len(),
                &mut length,
                buffer.as_mut_ptr(),
            )
        };
        if status != 0 || length != 1 {
            return None;
        }
        char::from_u32(u32::from(buffer[0]))
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) fn with_translator<T>(
        _run: impl FnOnce(&dyn Fn(u16) -> Option<char>) -> T,
    ) -> Option<T> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::find_code;

    fn jis(code: u16) -> Option<char> {
        match code {
            30 => Some('['),
            33 => Some('@'),
            42 => Some(']'),
            43 => Some(','),
            _ => None,
        }
    }

    #[test]
    fn finds_the_key_that_types_the_symbol_in_the_layout() {
        assert_eq!(find_code('[', jis), Some(30));
        assert_eq!(find_code(']', jis), Some(42));
        assert_eq!(find_code(',', jis), Some(43));
    }

    #[test]
    fn returns_none_when_no_key_types_the_symbol() {
        assert_eq!(find_code('`', jis), None);
    }
}
