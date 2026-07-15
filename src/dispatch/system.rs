use agent_desktop_core::{
    AppError, PermissionReport, PlatformAdapter,
    commands::{
        permissions as permissions_command, skills as skills_command, status as status_command,
        version as version_command, wait as wait_command,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    batch::BatchArgs,
    session::SessionArgs,
    skills::{SkillsAction, SkillsArgs},
    system::{PermissionsArgs, WaitArgs},
    trace::TraceArgs,
};
use crate::dispatch::{session as session_dispatch, trace as trace_dispatch};

pub(super) fn wait(
    args: WaitArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    wait_command::execute(
        wait_command::WaitArgs {
            mode: wait_command::WaitModeArgs {
                ms: args.mode.ms,
                element: args.mode.element,
                window: args.mode.window,
                text: args.mode.text,
                menu: args.mode.menu,
                menu_closed: args.mode.menu_closed,
                notification: args.mode.notification,
                event: args.event.event,
                window_id: args.event.window_id,
            },
            predicate: wait_command::WaitPredicateArgs {
                snapshot_id: args.predicate.snapshot,
                predicate: args.predicate.predicate,
                value: args.predicate.value,
                action: args.predicate.action,
                count: args.predicate.count,
            },
            timeout_ms: args.timeout,
            app: args.app,
        },
        adapter,
        context,
    )
}

pub(super) fn status(
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    status_command::execute_with_report_with_context(adapter, permission_report, context)
}

pub(super) fn permissions(
    args: PermissionsArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
) -> Result<Value, AppError> {
    permissions_command::execute_with_report(
        permissions_command::PermissionsArgs {
            request: args.request,
        },
        adapter,
        permission_report,
    )
}

pub(super) fn version() -> Result<Value, AppError> {
    version_command::execute()
}

pub(super) fn skills(args: SkillsArgs) -> Result<Value, AppError> {
    match args.action.unwrap_or(SkillsAction::List) {
        SkillsAction::List => skills_command::list(),
        SkillsAction::Path => skills_command::path(),
        SkillsAction::Get(get) => skills_command::get(skills_command::GetArgs {
            name: get.name,
            full: get.full,
            reference: get.reference,
        }),
    }
}

pub(super) fn session(args: SessionArgs, context: &CommandContext) -> Result<Value, AppError> {
    session_dispatch::dispatch(args, context)
}

pub(super) fn trace(args: TraceArgs, context: &CommandContext) -> Result<Value, AppError> {
    trace_dispatch::dispatch(args, context)
}

pub(super) fn batch(
    args: BatchArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    crate::batch::execute(args, adapter, permission_report, context)
}
