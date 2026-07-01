mod notifications;
mod parse;

use agent_desktop_core::{
    PermissionReport,
    adapter::PlatformAdapter,
    commands::{
        check, clear, click, clipboard_clear, clipboard_get, clipboard_set, close_app, collapse,
        double_click, drag, expand, find, focus, focus_window, get, helpers, hover, is_check,
        key_down, key_up, launch, list_apps, list_surfaces, list_windows, maximize, minimize,
        mouse_click, mouse_down, mouse_move, mouse_up, move_window, permissions, press,
        resize_window, restore, right_click, screenshot, scroll, scroll_to, select, session,
        set_value, skills, snapshot, status, toggle, triple_click, type_text, uncheck, version,
        wait,
    },
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;
use crate::cli_args::session::SessionAction;
use crate::cli_args::skills::SkillsAction;
use parse::{
    parse_direction, parse_get_property, parse_is_property, parse_mouse_button, parse_xy,
    parse_xy_opt,
};

pub(crate) fn dispatch(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    tracing::debug!("dispatch: {}", cmd.name());
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

        Commands::Find(a) => find::execute(
            find::FindArgs {
                app: a.app,
                role: a.role,
                name: a.name,
                value: a.value,
                text: a.text,
                count: a.count,
                first: a.first,
                last: a.last,
                nth: a.nth,
                limit: a.limit,
            },
            adapter,
            context,
        ),

        Commands::Screenshot(a) => screenshot::execute(
            screenshot::ScreenshotArgs {
                app: a.app,
                window_id: a.window_id,
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

        Commands::Click(a) => click::execute(ref_args(a), adapter, context),
        Commands::DoubleClick(a) => double_click::execute(ref_args(a), adapter, context),
        Commands::TripleClick(a) => triple_click::execute(ref_args(a), adapter, context),
        Commands::RightClick(a) => right_click::execute(ref_args(a), adapter, context),

        Commands::Type(a) => type_text::execute(
            type_text::TypeArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                text: a.text,
            },
            adapter,
            context,
        ),

        Commands::SetValue(a) => set_value::execute(
            set_value::SetValueArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                value: a.value,
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
            },
            adapter,
            context,
        ),

        Commands::ScrollTo(a) => scroll_to::execute(ref_args(a), adapter, context),

        Commands::Press(a) => press::execute(
            press::PressArgs {
                combo: a.combo,
                app: a.app,
                force: a.force,
            },
            adapter,
        ),

        Commands::KeyDown(a) => key_down::execute(
            key_down::KeyDownArgs {
                combo: a.combo,
                force: a.force,
            },
            adapter,
        ),

        Commands::KeyUp(a) => key_up::execute(
            key_up::KeyUpArgs {
                combo: a.combo,
                force: a.force,
            },
            adapter,
        ),

        Commands::Hover(a) => hover::execute(
            hover::HoverArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                xy: parse_xy_opt(a.xy.as_deref())?,
                duration_ms: a.duration,
            },
            adapter,
            context,
        ),

        Commands::Drag(a) => drag::execute(
            drag::DragArgs {
                from_ref: a.from,
                from_xy: parse_xy_opt(a.from_xy.as_deref())?,
                to_ref: a.to,
                to_xy: parse_xy_opt(a.to_xy.as_deref())?,
                snapshot_id: a.snapshot,
                duration_ms: a.duration,
                drop_delay_ms: a.drop_delay,
            },
            adapter,
            context,
        ),

        Commands::MouseMove(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_move::execute(mouse_move::MouseMoveArgs { x, y }, adapter, context)
        }

        Commands::MouseClick(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_click::execute(
                mouse_click::MouseClickArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                    count: a.count,
                },
                adapter,
                context,
            )
        }

        Commands::MouseDown(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_down::execute(
                mouse_down::MouseDownArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                },
                adapter,
                context,
            )
        }

        Commands::MouseUp(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_up::execute(
                mouse_up::MouseUpArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                },
                adapter,
                context,
            )
        }

        Commands::Launch(a) => launch::execute(
            launch::LaunchArgs {
                app: a.app,
                timeout_ms: a.timeout,
            },
            adapter,
        ),

        Commands::CloseApp(a) => close_app::execute(
            close_app::CloseAppArgs {
                app: a.app,
                force: a.force,
            },
            adapter,
        ),

        Commands::ListWindows(a) => {
            list_windows::execute(list_windows::ListWindowsArgs { app: a.app }, adapter)
        }

        Commands::ListApps(a) => {
            list_apps::execute(list_apps::ListAppsArgs { app: a.app }, adapter)
        }

        Commands::ListSurfaces(a) => {
            list_surfaces::execute(list_surfaces::ListSurfacesArgs { app: a.app }, adapter)
        }

        Commands::FocusWindow(a) => focus_window::execute(
            focus_window::FocusWindowArgs {
                window_id: a.window_id,
                app: a.app,
                title: a.title,
            },
            adapter,
        ),

        Commands::ResizeWindow(a) => resize_window::execute(
            resize_window::ResizeWindowArgs {
                app: a.app,
                width: a.width,
                height: a.height,
            },
            adapter,
        ),

        Commands::MoveWindow(a) => move_window::execute(
            move_window::MoveWindowArgs {
                app: a.app,
                x: a.x,
                y: a.y,
            },
            adapter,
        ),

        Commands::Minimize(a) => minimize::execute(helpers::AppArgs { app: a.app }, adapter),

        Commands::Maximize(a) => maximize::execute(helpers::AppArgs { app: a.app }, adapter),

        Commands::Restore(a) => restore::execute(helpers::AppArgs { app: a.app }, adapter),

        Commands::ListNotifications(_)
        | Commands::DismissNotification(_)
        | Commands::DismissAllNotifications(_)
        | Commands::NotificationAction(_) => notifications::dispatch_notification(cmd, adapter),

        Commands::ClipboardGet => clipboard_get::execute(adapter),
        Commands::ClipboardSet(a) => clipboard_set::execute(a.text, adapter),
        Commands::ClipboardClear => clipboard_clear::execute(adapter),

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

        Commands::Session(a) => match a.action {
            SessionAction::Start(s) => session::execute(session::SessionAction::Start {
                name: s.name,
                no_trace: s.no_trace,
                force: s.force,
            }),
            SessionAction::End(e) => session::execute(session::SessionAction::End { id: e.id }),
            SessionAction::List => session::execute(session::SessionAction::List),
            SessionAction::Gc(g) => session::execute(session::SessionAction::Gc {
                older_than_secs: g.older_than,
                ended_only: g.ended,
            }),
        },

        Commands::Batch(a) => crate::batch::execute(a, adapter, permission_report, context),
    }
}

fn ref_args(args: crate::cli_args::RefArgs) -> helpers::RefArgs {
    helpers::RefArgs {
        ref_id: args.ref_id,
        snapshot_id: args.snapshot_id,
    }
}
