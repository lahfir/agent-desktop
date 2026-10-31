use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;
use zbus::Connection;

use super::eis_link::EisLink;
use super::key_table::KeyTable;
use super::press_state::PressState;
use crate::input::keyboard::key_names::KEYSYM_SHIFT_L;
use crate::input::keyboard::keysym_step::KeysymStep;
use crate::input::wayland::key_session::KeySession;
use crate::system::bus::session_bus::SessionBus;
use crate::system::within;

pub(crate) const SERVICE: &str = "org.kde.KWin";
const PATH: &str = "/org/kde/KWin/EIS/RemoteDesktop";
const INTERFACE: &str = "org.kde.KWin.EIS.RemoteDesktop";
const LAYOUTS_PATH: &str = "/Layouts";
const LAYOUTS_INTERFACE: &str = "org.kde.KeyboardLayouts";
const KEYBOARD_CAPABILITY: i32 = 1;
const DISCONNECT_BUDGET_MS: u64 = 1_000;

/// A keyboard delivery through KWin's EIS input service. KWin hands out a
/// libei socket over the session bus; keysyms become evdev keys through the
/// keymap KWin sends with the virtual keyboard.
pub(crate) struct KwinKeyboard {
    connection: Connection,
    cookie: Option<i32>,
    link: Option<EisLink>,
    table: KeyTable,
    steps: Vec<KeysymStep>,
    state: PressState,
}

impl KwinKeyboard {
    /// Opens the session bus for a delivery that will send `steps`.
    pub(crate) fn open(steps: Vec<KeysymStep>, deadline: Deadline) -> Result<Self, AdapterError> {
        Ok(Self {
            connection: SessionBus::get(deadline)?,
            cookie: None,
            link: None,
            table: KeyTable::default(),
            steps,
            state: PressState::default(),
        })
    }

    fn link(&mut self) -> Result<&mut EisLink, AdapterError> {
        self.link
            .as_mut()
            .ok_or_else(|| AdapterError::internal("The KWin keyboard was not created"))
    }

    /// The layout KWin has active, which is the keymap group its keys use.
    fn layout(&self, deadline: Deadline) -> Result<u32, AdapterError> {
        within(deadline, async {
            self.connection
                .call_method(
                    Some(SERVICE),
                    LAYOUTS_PATH,
                    Some(LAYOUTS_INTERFACE),
                    "getLayout",
                    &(),
                )
                .await
                .map_err(|error| bus_failure(&error))?
                .body()
                .deserialize::<u32>()
                .map_err(|error| bus_failure(&error))
        })
    }

    fn disconnect(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let Some(cookie) = self.cookie.take() else {
            return Ok(());
        };
        self.link = None;
        within(deadline, async {
            self.connection
                .call_method(
                    Some(SERVICE),
                    PATH,
                    Some(INTERFACE),
                    "disconnect",
                    &(cookie,),
                )
                .await
                .map(|_| ())
                .map_err(|error| bus_failure(&error))
        })
    }
}

impl KeySession for KwinKeyboard {
    fn create(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let (fd, cookie) = within(deadline, async {
            self.connection
                .call_method(
                    Some(SERVICE),
                    PATH,
                    Some(INTERFACE),
                    "connectToEIS",
                    &(KEYBOARD_CAPABILITY,),
                )
                .await
                .map_err(|error| bus_failure(&error))?
                .body()
                .deserialize::<(zbus::zvariant::OwnedFd, i32)>()
                .map_err(|error| bus_failure(&error))
        })?;
        self.cookie = Some(cookie);
        let stream = UnixStream::from(OwnedFd::from(fd));
        let mut link = EisLink::connect(stream, deadline)?;
        let keymap = link.await_keyboard(deadline)?;
        self.table = KeyTable::from_keymap(&keymap, self.layout(deadline)?)?;
        self.link = Some(link);
        let mut rehearsal = PressState::default();
        let complete = self.table.lookup(KEYSYM_SHIFT_L).is_some()
            && self.steps.iter().all(|step| {
                self.table
                    .plan(step.keysym, step.down, &mut rehearsal)
                    .is_some()
            });
        if !complete {
            return Err(outside_layout());
        }
        Ok(())
    }

    fn start(&mut self, _deadline: Deadline) -> Result<(), AdapterError> {
        self.link()?.start()
    }

    fn keysym(&mut self, keysym: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError> {
        let presses = self
            .table
            .plan(keysym, down, &mut self.state)
            .ok_or_else(outside_layout)?;
        let link = self.link()?;
        for (code, down) in presses {
            link.key(code, down, deadline)?;
        }
        Ok(())
    }

    fn stop(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let stopped = self.link().and_then(EisLink::stop);
        self.disconnect(deadline).and(stopped)
    }
}

impl Drop for KwinKeyboard {
    fn drop(&mut self) {
        if let Ok(deadline) = Deadline::detached_after(DISCONNECT_BUDGET_MS) {
            let _ = self.disconnect(deadline);
        }
    }
}

fn outside_layout() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "A character in the text is outside the layout of the Wayland session keyboard",
    )
    .with_suggestion("Use the semantic set-value on the element instead of typing the text")
    .with_details(json!({
        "physical_delivery_started": false,
        "cause": "character_outside_layout",
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn bus_failure(error: &zbus::Error) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Keyboard input through KWin is not available in this session",
    )
    .with_suggestion("Use the semantic action on the element instead of physical input")
    .with_platform_detail(error.to_string())
}
