use super::keystroke_units;

#[test]
fn plain_text_types_one_character_per_keystroke() {
    assert_eq!(
        keystroke_units("hi there"),
        ["h", "i", " ", "t", "h", "e", "r", "e"]
    );
}

#[test]
fn a_combining_mark_types_with_its_base_character() {
    assert_eq!(
        keystroke_units("e\u{0301}t\u{00E9}"),
        ["e\u{0301}", "t", "\u{00E9}"]
    );
}

#[test]
fn joined_emoji_and_modifiers_type_as_one_keystroke() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let wave = "\u{1F44B}\u{1F3FD}";
    assert_eq!(
        keystroke_units(&format!("{family}{wave}!")),
        [family, wave, "!"]
    );
}

#[test]
fn crlf_types_as_one_keystroke_and_lf_alone_stays_alone() {
    assert_eq!(keystroke_units("a\r\nb\nc"), ["a", "\r\n", "b", "\n", "c"]);
}
