use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) fn key_name_to_code(key: &str) -> Result<u16, AdapterError> {
    let code = match key {
        "a" => 0,
        "b" => 11,
        "c" => 8,
        "d" => 2,
        "e" => 14,
        "f" => 3,
        "g" => 5,
        "h" => 4,
        "i" => 34,
        "j" => 38,
        "k" => 40,
        "l" => 37,
        "m" => 46,
        "n" => 45,
        "o" => 31,
        "p" => 35,
        "q" => 12,
        "r" => 15,
        "s" => 1,
        "t" => 17,
        "u" => 32,
        "v" => 9,
        "w" => 13,
        "x" => 7,
        "y" => 16,
        "z" => 6,
        "0" => 29,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,
        "return" | "enter" => 36,
        "escape" | "esc" => 53,
        "tab" => 48,
        "space" => 49,
        "delete" | "backspace" => 51,
        "forwarddelete" => 117,
        "home" => 115,
        "end" => 119,
        "pageup" => 116,
        "pagedown" => 121,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        "cmd" | "command" => 55,
        "shift" => 56,
        "alt" | "option" => 58,
        "ctrl" | "control" => 59,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        other => {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("Unknown key: '{other}'"),
            )
            .with_suggestion("Valid keys: a-z, 0-9, return, escape, tab, space, delete, left, right, up, down, f1-f12"))
        }
    };
    Ok(code)
}

#[cfg(test)]
mod tests {
    use agent_desktop_core::ErrorCode;

    use super::key_name_to_code;

    #[test]
    fn named_key_aliases_resolve_to_same_code() {
        assert_eq!(key_name_to_code("return").unwrap(), 36);
        assert_eq!(key_name_to_code("enter").unwrap(), 36);
        assert_eq!(key_name_to_code("escape").unwrap(), 53);
        assert_eq!(key_name_to_code("esc").unwrap(), 53);
        assert_eq!(key_name_to_code("alt").unwrap(), 58);
        assert_eq!(key_name_to_code("option").unwrap(), 58);
        assert_eq!(key_name_to_code("cmd").unwrap(), 55);
        assert_eq!(key_name_to_code("command").unwrap(), 55);
        assert_eq!(key_name_to_code("ctrl").unwrap(), 59);
        assert_eq!(key_name_to_code("control").unwrap(), 59);
    }

    #[test]
    fn navigation_and_function_keys_map_to_expected_codes() {
        assert_eq!(key_name_to_code("f1").unwrap(), 122);
        assert_eq!(key_name_to_code("f12").unwrap(), 111);
        assert_eq!(key_name_to_code("tab").unwrap(), 48);
        assert_eq!(key_name_to_code("delete").unwrap(), 51);
        assert_eq!(key_name_to_code("backspace").unwrap(), 51);
        assert_eq!(key_name_to_code("left").unwrap(), 123);
        assert_eq!(key_name_to_code("up").unwrap(), 126);
    }

    #[test]
    fn unknown_key_name_returns_invalid_args_error_with_suggestion() {
        let err = key_name_to_code("hyperkey").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgs);
        assert!(err.message.contains("hyperkey"));
        assert!(err.suggestion.is_some());
    }
}
