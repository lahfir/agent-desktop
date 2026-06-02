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
        if let Some(ms) = args.ms {
            return Ok(Self::Sleep(ms));
        }
        if args.menu || args.menu_closed {
            return Ok(Self::Menu {
                app: args.app,
                open: args.menu,
            });
        }
        if args.notification {
            return Ok(Self::Notification {
                app: args.app,
                text: args.text,
            });
        }
        if let Some(ref_id) = args.element {
            validate_ref_id(&ref_id)?;
            let predicate =
                wait_predicate::ElementPredicate::parse(args.predicate.as_deref(), args.value)?;
            return Ok(Self::Element {
                ref_id,
                snapshot_id: args.snapshot_id,
                predicate,
            });
        }
        if let Some(title) = args.window {
            return Ok(Self::Window(title));
        }
        if let Some(text) = args.text {
            return Ok(Self::Text {
                text,
                count: args.count,
                app: args.app,
            });
        }
        Err(missing_wait_mode())
    }
}

pub(crate) fn validate_wait_mode(args: &WaitArgs) -> Result<(), AppError> {
    if args.predicate.is_some() && args.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--predicate requires --element",
            "Use --element <ref> with --predicate, or remove --predicate.",
        ));
    }
    if args.value.is_some() && args.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--value requires --element and --predicate value",
            "Use --element <ref> --predicate value --value <expected>.",
        ));
    }
    if args.count.is_some() && (args.text.is_none() || args.notification) {
        return Err(AppError::invalid_input_with_suggestion(
            "--count is only valid for --text waits",
            "Use --text <text> --count <expected> without --notification, or remove --count.",
        ));
    }
    let selected = [
        args.ms.is_some(),
        args.element.is_some(),
        args.window.is_some(),
        args.text.is_some() && !args.notification,
        args.menu,
        args.menu_closed,
        args.notification,
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
