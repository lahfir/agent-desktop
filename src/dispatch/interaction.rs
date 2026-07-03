use agent_desktop_core::{
    commands::{
        check, clear, click, collapse, double_click, expand, focus, helpers, right_click, scroll,
        scroll_to, select, set_value, toggle, triple_click, type_text, uncheck,
    },
    context::CommandContext,
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;
use crate::dispatch::parse::parse_direction;

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Click(a) => click::execute(ref_args(a), adapter, context),
        Commands::DoubleClick(a) => double_click::execute(ref_args(a), adapter, context),
        Commands::TripleClick(a) => triple_click::execute(ref_args(a), adapter, context),
        Commands::RightClick(a) => right_click::execute(ref_args(a), adapter, context),

        Commands::Type(a) => type_text::execute(
            type_text::TypeArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                text: a.text,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::SetValue(a) => set_value::execute(
            set_value::SetValueArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                value: a.value,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::Clear(a) => clear::execute(ref_args(a), adapter, context),

        Commands::Focus(a) => focus::execute(ref_args(a), adapter, context),
        Commands::Toggle(a) => toggle::execute(ref_args(a), adapter, context),
        Commands::Check(a) => check::execute(ref_args(a), adapter, context),
        Commands::Uncheck(a) => uncheck::execute(ref_args(a), adapter, context),
        Commands::Expand(a) => expand::execute(ref_args(a), adapter, context),
        Commands::Collapse(a) => collapse::execute(ref_args(a), adapter, context),

        Commands::Select(a) => select::execute(
            select::SelectArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                value: a.value,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::Scroll(a) => scroll::execute(
            scroll::ScrollArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                direction: parse_direction(&a.direction)?,
                amount: a.amount,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::ScrollTo(a) => scroll_to::execute(ref_args(a), adapter, context),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "interaction::dispatch received a non-interaction command",
            ),
        )),
    }
}

fn ref_args(args: crate::cli_args::RefArgs) -> helpers::RefArgs {
    helpers::RefArgs {
        ref_id: args.ref_id,
        snapshot_id: args.snapshot_id,
        timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
    }
}
