//! The renderer: one process, one window, one pipe, for as long as its
//! session lasts.
//!
//! Entered before argument parsing, so a process launched as the child never
//! reaches the CLI. It claims its pipe **first** and creates its window
//! second: the pipe is the singleton lock, and a child that loses that race
//! has to withdraw before anything is drawn, or a duplicate overlay flashes
//! on screen. A child whose window then fails to appear exits too, releasing
//! the name so the next enable can start a replacement rather than
//! connecting forever to a renderer that draws nothing.

use agent_desktop_core::AdapterError;

use super::pipe_name;

/// `Some` when this process was launched as the overlay's renderer. Checked
/// ahead of clap, which is why the child never parses a command line.
pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    let marker = std::env::var(pipe_name::CHILD_MARKER).ok()?;
    if marker != pipe_name::PROTOCOL_GENERATION {
        return Some(Err(AdapterError::internal(
            "The cursor overlay child was started for a different protocol generation",
        )));
    }
    let arguments: Vec<String> = std::env::args().collect();
    let (session_id, _) = pipe_name::parse_child_arguments(&arguments)?;
    Some(run(&session_id))
}

#[cfg(target_os = "windows")]
fn run(session_id: &str) -> Result<(), AdapterError> {
    imp::run(session_id)
}

#[cfg(not(target_os = "windows"))]
fn run(_session_id: &str) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("run_cursor_overlay_child"))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::pipe_name;
    use crate::system::cursor_overlay::{framing, server, surface_host};
    use agent_desktop_core::{AdapterError, ErrorCode};

    pub(super) fn run(session_id: &str) -> Result<(), AdapterError> {
        crate::system::dpi::ensure_per_monitor_v2()?;

        let root = agent_desktop_core::session::agent_desktop_dir()
            .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
        let name = pipe_name::pipe_name(&root, session_id);

        let listener = match server::Listener::claim(&name) {
            Ok(listener) => listener,
            Err(server::ClaimError::AlreadyServed) => return Ok(()),
            Err(server::ClaimError::Failed(error)) => return Err(error),
        };

        let mut host = surface_host::SurfaceHost::create(session_id.to_owned())?;

        serve(&listener, &mut host, session_id)
    }

    /// The connection is released before anything that only has to look right
    /// is drawn.
    ///
    /// A control is answered as soon as what its sender waits for is true, and
    /// the pipe is disconnected immediately after, so the next client is
    /// already being accepted while the card is still easing in and the click
    /// flourish is still playing. Holding the connection for the whole
    /// animation left a second overlaid action connecting to a busy pipe for
    /// the better part of a second - longer than the budget it had - and
    /// reporting that nothing had rendered.
    ///
    /// A `Disable` never settles: the process is about to exit, and there is
    /// nothing left to draw onto.
    fn serve(
        listener: &server::Listener,
        host: &mut surface_host::SurfaceHost,
        session_id: &str,
    ) -> Result<(), AdapterError> {
        loop {
            match listener.next_control(host.idle_tick()) {
                server::Accepted::Control(control) => {
                    let ours = control.session_id() == session_id;
                    if ours {
                        host.apply(&control);
                        if framing::is_acknowledged(&control) {
                            listener.acknowledge();
                        }
                    }
                    listener.finish();
                    if ours && control.is_disable() {
                        return Ok(());
                    }
                    if ours {
                        host.settle(&control, &|| listener.control_waiting());
                    }
                }
                server::Accepted::Idle => {
                    host.rest();
                    if host.session_finished() {
                        return Ok(());
                    }
                }
                server::Accepted::Broken(error) => return Err(error),
            }
        }
    }
}
