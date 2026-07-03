use agent_desktop_core::{
    PermissionReport,
    commands::{permissions, skills, status, version, wait},
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;
use crate::cli_args::skills::SkillsAction;
use crate::dispatch::{session, trace};

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Wait(a) => wait::execute(
            wait::WaitArgs {
                mode: wait::WaitModeArgs {
                    ms: a.mode.ms,
                    element: a.mode.element,
                    window: a.mode.window,
                    text: a.mode.text,
                    menu: a.mode.menu,
                    menu_closed: a.mode.menu_closed,
                    notification: a.mode.notification,
                    event: a.event.event,
                    window_id: a.event.window_id,
                },
                predicate: wait::WaitPredicateArgs {
                    snapshot_id: a.predicate.snapshot,
                    predicate: a.predicate.predicate,
                    value: a.predicate.value,
                    action: a.predicate.action,
                    count: a.predicate.count,
                },
                timeout_ms: a.timeout,
                app: a.app,
            },
            adapter,
            context,
        ),

        Commands::Status => {
            status::execute_with_report_with_context(adapter, permission_report, context)
        }

        Commands::Permissions(a) => permissions::execute_with_report(
            permissions::PermissionsArgs { request: a.request },
            adapter,
            permission_report,
        ),

        Commands::Version => version::execute(),

        Commands::Skills(a) => match a.action.unwrap_or(SkillsAction::List) {
            SkillsAction::List => skills::list(),
            SkillsAction::Path => skills::path(),
            SkillsAction::Get(g) => skills::get(skills::GetArgs {
                name: g.name,
                full: g.full,
                reference: g.reference,
            }),
        },

        Commands::Session(a) => session::dispatch(a, context),

        Commands::Trace(a) => trace::dispatch(a, context),

        Commands::Batch(a) => crate::batch::execute(a, adapter, permission_report, context),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "system::dispatch received a non-system command",
            ),
        )),
    }
}
