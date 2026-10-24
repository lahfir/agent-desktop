use std::time::{Duration, Instant};

use agent_desktop_core::{
    Action, ActionRequest, DeliverySemantics, InteractionLease, ObservationOps, RefEntry,
    execute_verified_action,
};

use super::menu_live_rig::{Rig, deadline, is_closed_ref, poll};
use crate::system::ACCESSIBLE_INTERFACE;
use crate::tree::live::live_launch::{Launched, nonce};
use crate::tree::{AtspiElement, atspi_element};

const CHECKED_BIT: u32 = 4;
const ITEM: &str = "Text Wrapping";

fn checked(rig: &Rig, element: &AtspiElement) -> bool {
    let reply = rig
        .connection
        .call_method(
            Some(element.bus_name()),
            element.path(),
            Some(ACCESSIBLE_INTERFACE),
            "GetState",
            &(),
        )
        .expect("GetState answers");
    reply
        .body()
        .deserialize::<Vec<u32>>()
        .expect("state words")
        .first()
        .is_some_and(|word| word & (1 << CHECKED_BIT) != 0)
}

struct Editor {
    rig: Rig,
    view: AtspiElement,
    entry: RefEntry,
}

fn open_view_menu(launched: &mut Launched) -> Editor {
    let dir = std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("a private session");
    let file = format!("{dir}/{}.txt", nonce("note"));
    std::fs::write(&file, "alpha beta gamma\n").expect("a note");
    let pid = launched.spawn("gnome-text-editor", &[&file]);
    let rig = Rig::attach(pid);
    let window = rig.window();
    let opener = poll("the View toggle", || {
        rig.window_entry(&window, "switch", "View")
    });
    let view = rig.resolve(&opener).expect("the View toggle resolves");
    rig.do_action(&view);
    poll("a menu surface", || rig.menu_surface());
    let entries = rig.menu_entries(&window).expect("the menu snapshot");
    let entry = entries
        .into_iter()
        .map(|(_, entry)| entry)
        .find(|entry| entry.identity.name.as_deref() == Some(ITEM))
        .unwrap_or_else(|| panic!("no item {ITEM:?} in the View menu"));
    Editor { rig, view, entry }
}

#[test]
#[ignore = "drives real desktop applications; run through scripts/private-session.sh --wm"]
fn menu_live_gtk4_check_item_toggle_is_verified_after_its_popup_closes() {
    let mut launched = Launched::new();
    let Editor { rig, view, entry } = open_view_menu(&mut launched);
    let handle = rig
        .adapter
        .resolve_element_strict(&entry, deadline())
        .expect("the ref resolves while its menu is open");
    let item = atspi_element(&handle).expect("an element").clone();
    let before = checked(&rig, &item);
    let lease = InteractionLease::guarded(deadline(), ()).expect("a lease");

    let result = execute_verified_action(
        &rig.adapter,
        &handle,
        ActionRequest::headless(Action::Toggle),
        &lease,
    )
    .expect("a toggle the adapter verified is not failed by a read of the closed popup");

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    poll("the menu to close", || {
        rig.menu_surface().is_none().then_some(())
    });
    rig.do_action(&view);
    poll("a menu surface", || rig.menu_surface());
    let reopened = rig
        .resolve(&entry)
        .expect("the reopened menu answers the ref");
    assert_eq!(
        checked(&rig, &reopened),
        !before,
        "an independent read sees the check item flipped"
    );
}

#[test]
#[ignore = "drives real desktop applications; run through scripts/private-session.sh --wm"]
fn menu_live_gtk4_item_ref_is_closed_menu_stale_as_soon_as_its_popup_goes() {
    let mut launched = Launched::new();
    let Editor { rig, entry, .. } = open_view_menu(&mut launched);
    let item = rig.resolve(&entry).expect("the ref resolves while open");
    rig.do_action(&item);

    let started = Instant::now();
    let error = loop {
        match rig.resolve(&entry) {
            Ok(_) if started.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(_) => panic!("the ref still resolved five seconds after its popup closed"),
            Err(error) => break error,
        }
    };

    assert!(
        is_closed_ref(&error),
        "the popup going away reads as a closed menu and nothing else: {error:?}"
    );
}
