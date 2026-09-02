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
    use crate::system::cursor_overlay::{framing, server, session_state, surface_host};
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
        watch_session(session_id.to_owned());

        serve(&listener, &mut host, session_id)
    }

    /// Ends the renderer when its session does, whether or not a `Disable`
    /// ever arrives.
    ///
    /// It runs on its own thread because the accept below blocks until a
    /// client connects - which is correct for a server waiting for work and
    /// useless as a place to notice that the work will never come. A crashed
    /// agent, a `session gc`, or an operator who simply stops would otherwise
    /// leave a topmost animated window with nothing in the product able to
    /// remove it.
    fn watch_session(session_id: String) {
        std::thread::spawn(move || {
            let mut watch = session_state::EndWatch::default();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(
                    agent_desktop_core::CURSOR_IDLE_REST_MS,
                ));
                let reading = session_state::classify(session_state::read_manifest(&session_id));
                if watch.observe(reading) {
                    std::process::exit(0);
                }
            }
        });
    }

    fn serve(
        listener: &server::Listener,
        host: &mut surface_host::SurfaceHost,
        session_id: &str,
    ) -> Result<(), AdapterError> {
        loop {
            match listener.accept_next(host.idle_tick()) {
                server::Accepted::Control(connection, control) => {
                    if control.session_id() != session_id {
                        continue;
                    }
                    let disabling = control.is_disable();
                    host.apply(&control);
                    if framing::is_acknowledged(&control) {
                        connection.acknowledge();
                    }
                    if disabling {
                        return Ok(());
                    }
                }
                server::Accepted::Refused => {}
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
