pub(crate) mod eis_link;
pub(crate) mod evdev_key;
pub(crate) mod key_table;
pub(crate) mod kwin_keyboard;
pub(crate) mod press_state;
pub(crate) mod xkb_library;

use agent_desktop_core::{AdapterError, Deadline};
use zbus::Connection;

use super::remote_desktop::{SERVICE as MUTTER, classify_call};
use crate::system::bus::session_bus::SessionBus;
use crate::system::within;

const BUS_DAEMON: &str = "org.freedesktop.DBus";
const BUS_DAEMON_PATH: &str = "/org/freedesktop/DBus";

/// Whether keyboard delivery goes through KWin: the session bus has KWin and
/// no Mutter RemoteDesktop, which keeps GNOME on its own route.
pub(crate) fn serves(deadline: Deadline) -> Result<bool, AdapterError> {
    let connection = SessionBus::get(deadline)?;
    within(deadline, async {
        Ok(!has_owner(&connection, MUTTER).await?
            && has_owner(&connection, kwin_keyboard::SERVICE).await?)
    })
}

async fn has_owner(connection: &Connection, name: &str) -> Result<bool, AdapterError> {
    connection
        .call_method(
            Some(BUS_DAEMON),
            BUS_DAEMON_PATH,
            Some(BUS_DAEMON),
            "NameHasOwner",
            &(name,),
        )
        .await
        .and_then(|reply| reply.body().deserialize::<bool>())
        .map_err(|error| classify_call(&error))
}

#[cfg(test)]
mod live_tests;
