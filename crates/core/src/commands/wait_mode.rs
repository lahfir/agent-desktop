use crate::{
    commands::{wait::WaitArgs, wait_predicate},
    error::AppError,
    refs::validate_ref_id,
};

pub(crate) enum WaitMode {
    Sleep(u64),
    Element {
        ref_id: String,
        snapshot_id: Option<String>,
        predicate: wait_predicate::ElementPredicate,
    },
    Window(String),
    Text {
        text: String,
        count: Option<usize>,
        app: Option<String>,
    },
    Menu {
        app: Option<String>,
        open: bool,
    },
    Notification {
        app: Option<String>,
        text: Option<String>,
    },
}

impl WaitMode {
    pub(crate) fn from_args(args: WaitArgs) -> Result<Self, AppError> {
        validate_wait_mode(&args)?;
        if let Some(ms) = args.mode.ms {
            return Ok(Self::Sleep(ms));
        }
        if args.mode.menu || args.mode.menu_closed {
            return Ok(Self::Menu {
                app: args.app,
                open: args.mode.menu,
            });
        }
        if args.mode.notification {
            return Ok(Self::Notification {
                app: args.app,
                text: args.mode.text,
            });
        }
        if let Some(ref_id) = args.mode.element {
            validate_ref_id(&ref_id)?;
            let predicate = wait_predicate::ElementPredicate::parse(
                args.predicate.predicate.as_deref(),
                args.predicate.value,
            )?;
            return Ok(Self::Element {
                ref_id,
                snapshot_id: args.predicate.snapshot_id,
                predicate,
            });
        }
        if let Some(title) = args.mode.window {
            return Ok(Self::Window(title));
        }
        if let Some(text) = args.mode.text {
            return Ok(Self::Text {
                text,
                count: args.predicate.count,
                app: args.app,
            });
        }
        Err(missing_wait_mode())
    }
}

pub(crate) fn validate_wait_mode(args: &WaitArgs) -> Result<(), AppError> {
    if args.predicate.predicate.is_some() && args.mode.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--predicate requires --element",
            "Use --element <ref> with --predicate, or remove --predicate.",
        ));
    }
    if args.predicate.value.is_some() && args.mode.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--value requires --element and --predicate value",
            "Use --element <ref> --predicate value --value <expected>.",
        ));
    }
    if args.predicate.count.is_some() && (args.mode.text.is_none() || args.mode.notification) {
        return Err(AppError::invalid_input_with_suggestion(
            "--count is only valid for --text waits",
            "Use --text <text> --count <expected> without --notification, or remove --count.",
        ));
    }
    let selected = [
        args.mode.ms.is_some(),
        args.mode.element.is_some(),
        args.mode.window.is_some(),
        args.mode.text.is_some() && !args.mode.notification,
        args.mode.menu,
        args.mode.menu_closed,
        args.mode.notification,
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selected == 1 {
        return Ok(());
    }
    if selected == 0 {
        return Err(missing_wait_mode());
    }
    Err(AppError::invalid_input_with_suggestion(
        "wait accepts exactly one mode",
        "Use one of: ms, --element, --window, --text, --menu, --menu-closed, or --notification.",
    ))
}

fn missing_wait_mode() -> AppError {
    AppError::invalid_input(
        "Provide a duration (ms), --menu, --notification, --element <ref>, --window <title>, or --text <text>",
    )
}
