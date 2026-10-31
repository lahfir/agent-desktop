use std::time::{Duration, Instant};

use agent_desktop_core::Deadline;

use super::kwin_keyboard::KwinKeyboard;
use super::serves;
use crate::input::keyboard::key_names::KEYSYM_RETURN;
use crate::input::wayland::key_session::KeySession;
use crate::tree::live::live_launch::Launched;

const KEYSYM_UPPER_A: u32 = 0x41;
const START_WAIT: Duration = Duration::from_secs(20);

fn deadline() -> Deadline {
    Deadline::after(20_000).expect("a deadline builds")
}

#[test]
#[ignore = "starts a headless kwin_wayland; run inside scripts/private-session.sh"]
fn a_headless_kwin_takes_keys_through_its_eis_service() {
    let mut launched = Launched::new();
    launched.spawn(
        "kwin_wayland",
        &[
            "--virtual",
            "--no-lockscreen",
            "--no-global-shortcuts",
            "--socket",
            "wayland-kwin",
        ],
    );
    let until = Instant::now() + START_WAIT;
    while !serves(deadline()).unwrap_or(false) {
        assert!(
            Instant::now() < until,
            "the headless KWin did not take the session bus"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let wanted = vec![KEYSYM_UPPER_A, KEYSYM_RETURN];
    let mut keyboard = KwinKeyboard::open(wanted, deadline()).expect("the session bus opens");
    let created = (0..100).find_map(|_| match keyboard.create(deadline()) {
        Ok(()) => Some(()),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(100));
            None
        }
    });
    created.expect("KWin hands out a resumed keyboard");
    keyboard.start(deadline()).expect("emulation starts");
    for (keysym, down) in [
        (KEYSYM_UPPER_A, true),
        (KEYSYM_UPPER_A, false),
        (KEYSYM_RETURN, true),
        (KEYSYM_RETURN, false),
    ] {
        keyboard
            .keysym(keysym, down, deadline())
            .expect("KWin confirms each key");
    }
    keyboard.stop(deadline()).expect("the keyboard stops");
}
