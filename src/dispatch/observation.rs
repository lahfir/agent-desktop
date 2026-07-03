use agent_desktop_core::{
    commands::{find, get, is_check, screenshot, snapshot},
    context::CommandContext,
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;
use crate::dispatch::parse::{parse_get_property, parse_is_property};

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Snapshot(a) => snapshot::execute(
            snapshot::SnapshotArgs {
                app: a.app,
                window_id: a.window_id,
                max_depth: a.max_depth,
                include_bounds: a.include_bounds,
                interactive_only: a.interactive_only,
                compact: a.compact,
                surface: a.surface.to_core(),
                skeleton: a.skeleton,
                root_ref: a.root,
                snapshot_id: a.snapshot,
            },
            adapter,
            context,
        ),

        Commands::Find(a) => {
            let states = a
                .states
                .iter()
                .map(|raw| find::parse_state_flag(raw))
                .collect::<Result<Vec<_>, _>>()?;
            find::execute(
                find::FindArgs {
                    app: a.app,
                    filter: find::FindFilterArgs {
                        role: a.filter.role,
                        name: a.filter.name,
                        description: a.filter.description,
                        native_id: a.filter.native_id,
                        value: a.filter.value,
                        text: a.filter.text,
                        exact: a.filter.exact,
                    },
                    states,
                    selection: find::FindSelectionArgs {
                        count: a.selection.count,
                        first: a.selection.first,
                        last: a.selection.last,
                        nth: a.selection.nth,
                        limit: a.selection.limit,
                    },
                },
                adapter,
                context,
            )
        }

        Commands::Screenshot(a) => screenshot::execute(
            screenshot::ScreenshotArgs {
                app: a.app,
                window_id: a.window_id,
                screen: a.screen,
                output_path: a.output_path,
            },
            adapter,
        ),

        Commands::Get(a) => get::execute(
            get::GetArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                property: parse_get_property(&a.property)?,
            },
            adapter,
            context,
        ),

        Commands::Is(a) => is_check::execute(
            is_check::IsArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                property: parse_is_property(&a.property)?,
            },
            adapter,
            context,
        ),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "observation::dispatch received a non-observation command",
            ),
        )),
    }
}
