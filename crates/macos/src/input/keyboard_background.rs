use agent_desktop_core::{
    AdapterError, BackgroundDeliveryReport, BackgroundKeyInput, Deadline, WindowInfo,
};

use crate::actions::DeliveryTracker;
use crate::input::background_delivery::{
    Prepared, SystemDeliveryIo, note_once, run, window_number,
};
use crate::input::background_key_events::{KeyRouting, build, plan};
use crate::input::background_layers::BackgroundLayers;
use crate::input::prepared_event::PreparedEvent;
use crate::input::skylight;

/// Delivers keys to the process that owns `window` without activating it,
/// so the user's frontmost app keeps its key window and the pointer stays
/// put. Menu shortcuts are never looked up and the focused element is never
/// read: the keys are the only thing sent, and the target app decides what
/// they do (it may still resolve a Command combo to one of its own menu
/// items).
///
/// AppKit sends key events to its key window, and an inactive app's windows
/// are not key, so [`BackgroundLayers`] (from `AGENT_DESKTOP_BG_LAYERS`) add,
/// by default: `keywindow`, target-only records that make this window key
/// inside its own process; `activate`, the target-only focus record the
/// pointer path uses; `route`, the target pid and window number on every
/// event; `skylight`, posting through `SLEventPostToPid`; `auth`, the
/// authentication message Chromium needs on synthetic keys; and `guard`,
/// which restores the user's app if the target activates itself. The effect must
/// be observed with a fresh snapshot.
pub(crate) fn deliver(
    window: &WindowInfo,
    input: &BackgroundKeyInput,
    deadline: Deadline,
) -> Result<BackgroundDeliveryReport, AdapterError> {
    let prepared =
        prepare(window, input).map_err(|error| DeliveryTracker::default().annotate(error))?;
    let mut io = SystemDeliveryIo::new(deadline);
    let mut report = run(prepared, deadline, &mut io)?;
    io.note_degradations(&mut report.degradations);
    Ok(report)
}

/// Everything that can fail runs here, before anything is posted, so those
/// failures are reported as not delivered.
fn prepare(window: &WindowInfo, input: &BackgroundKeyInput) -> Result<Prepared, AdapterError> {
    let layers = BackgroundLayers::keyboard_from_env()?;
    let window_number = window_number(window)?;
    let pid = crate::system::process_identity::to_pid_t(window.pid)?;
    let routing = KeyRouting {
        pid,
        window_number: i64::from(window_number),
        route: layers.route,
    };

    let planned = plan(input)?;
    let mut events = Vec::with_capacity(planned.len());
    let mut degradations = Vec::new();
    for key in &planned {
        let event = build(key, &routing)?;
        if layers.auth && !skylight::authenticate(&event, pid) {
            note_once(
                &mut degradations,
                "auth:SLSEventAuthenticationMessage_unavailable",
            );
        }
        events.push(PreparedEvent {
            event,
            pause_after: key.pause_after,
            completes_press: !key.down,
        });
    }

    Ok(Prepared {
        pid,
        window_number,
        layers,
        events,
        degradations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::KeyCombo;

    #[test]
    fn failures_before_posting_are_reported_as_not_delivered() {
        let window = |id: &str| WindowInfo {
            id: id.to_string(),
            title: String::new(),
            app: "Code".to_string(),
            pid: agent_desktop_core::ProcessId::new(std::process::id()),
            process_instance: None,
            bounds: None,
            state: agent_desktop_core::WindowState::default(),
        };
        let unknown_key = BackgroundKeyInput::Combo(KeyCombo {
            key: "hyperkey".into(),
            modifiers: Vec::new(),
        });
        let text = BackgroundKeyInput::Text("hi".into());
        let cases = [
            (window("not-a-window"), &text),
            (window("w-99999999999"), &text),
            (window("w-15592"), &unknown_key),
        ];

        for (target, input) in cases {
            let deadline = Deadline::after(1_000).unwrap();
            let err = deliver(&target, input, deadline).expect_err("must fail before posting");

            assert_eq!(
                err.disposition,
                agent_desktop_core::DeliverySemantics::not_delivered()
            );
        }
    }
}
