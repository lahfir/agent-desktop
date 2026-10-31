use super::*;

fn chars_sent(messages: &[WindowMessage]) -> Vec<u16> {
    messages
        .iter()
        .filter(|message| message.msg == WM_CHAR)
        .map(|message| message.wparam as u16)
        .collect()
}

#[test]
fn each_utf16_unit_is_a_packet_down_char_and_packet_up() {
    let messages = text_messages("a中");

    assert_eq!(
        messages,
        vec![
            WindowMessage::new(WM_KEYDOWN, VK_PACKET, 1),
            WindowMessage::new(WM_CHAR, 'a' as usize, 1),
            WindowMessage::new(WM_KEYUP, VK_PACKET, 0xC000_0001),
            WindowMessage::new(WM_KEYDOWN, VK_PACKET, 1),
            WindowMessage::new(WM_CHAR, '中' as usize, 1),
            WindowMessage::new(WM_KEYUP, VK_PACKET, 0xC000_0001),
        ]
    );
}

#[test]
fn surrogate_pairs_are_sent_as_two_char_messages_in_order() {
    let text = "😀";
    let expected: Vec<u16> = text.encode_utf16().collect();

    assert_eq!(chars_sent(&text_messages(text)), expected);
    assert_eq!(expected.len(), 2);
}

#[test]
fn line_breaks_are_return_key_edges_around_one_carriage_return() {
    let messages = text_messages("\n");

    assert_eq!(
        messages,
        vec![
            WindowMessage::new(WM_KEYDOWN, VK_RETURN, 0x001C_0001),
            WindowMessage::new(WM_CHAR, CARRIAGE_RETURN, 0x001C_0001),
            WindowMessage::new(WM_KEYUP, VK_RETURN, 0xC01C_0001),
        ]
    );
}

#[test]
fn crlf_is_one_line_break() {
    let returns = text_messages("a\r\nb")
        .iter()
        .filter(|message| message.msg == WM_KEYDOWN && message.wparam == VK_RETURN)
        .count();

    assert_eq!(returns, 1);
}

#[test]
fn every_text_message_carries_text_and_activation_does_not() {
    assert!(
        text_messages("x\ny")
            .iter()
            .all(WindowMessage::carries_text)
    );
    assert!(
        !activation_messages(true)
            .iter()
            .chain(activation_messages(false).iter())
            .chain(click_messages(1, 1).iter())
            .any(WindowMessage::carries_text)
    );
}

#[test]
fn activation_mirrors_deactivation() {
    let on = activation_messages(true);
    let off = activation_messages(false);

    assert_eq!(on[0], WindowMessage::new(WM_NCACTIVATE, 1, 0));
    assert_eq!(on[1], WindowMessage::new(WM_ACTIVATE, WA_ACTIVE, 0));
    assert_eq!(on[2].msg, WM_SETFOCUS);
    assert_eq!(off[0], WindowMessage::new(WM_NCACTIVATE, 0, 0));
    assert_eq!(off[1], WindowMessage::new(WM_ACTIVATE, WA_INACTIVE, 0));
    assert_eq!(off[2].msg, WM_KILLFOCUS);
}

#[test]
fn click_packs_client_coordinates_into_lparam() {
    let [moved, down, up] = click_messages(300, 40);

    assert_eq!(moved.lparam, (40 << 16) | 300);
    assert_eq!(
        down,
        WindowMessage::new(WM_LBUTTONDOWN, MK_LBUTTON, moved.lparam)
    );
    assert_eq!(up, WindowMessage::new(WM_LBUTTONUP, 0, moved.lparam));
}

#[test]
fn negative_client_coordinates_keep_their_low_sixteen_bits() {
    let [moved, _, _] = click_messages(-2, -3);

    assert_eq!(moved.lparam & 0xFFFF, 0xFFFE);
    assert_eq!((moved.lparam >> 16) & 0xFFFF, 0xFFFD);
}
