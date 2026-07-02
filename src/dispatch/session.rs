use agent_desktop_core::{commands::session, context::CommandContext, error::AppError};
use serde_json::Value;

use crate::cli_args::session::{SessionAction, SessionArgs};

pub(crate) fn dispatch(args: SessionArgs, context: &CommandContext) -> Result<Value, AppError> {
    match args.action {
        SessionAction::Start(s) => session::execute(session::SessionAction::Start {
            name: s.name,
            no_trace: s.no_trace,
            screenshots: s.screenshots,
            force: s.force,
        }),
        SessionAction::End(e) => session::execute(session::SessionAction::End {
            id: e.id.or_else(|| context.session_id().map(str::to_string)),
        }),
        SessionAction::List => session::execute(session::SessionAction::List),
        SessionAction::Gc(g) => session::execute(session::SessionAction::Gc {
            older_than_secs: g.older_than,
            ended_only: g.ended,
        }),
    }
}
