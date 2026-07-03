use agent_desktop_core::{
    commands::{clipboard_clear, clipboard_get, clipboard_set},
    context::CommandContext,
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;
use crate::dispatch::parse::parse_clipboard_format;

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::ClipboardGet(a) => clipboard_get::execute(
            clipboard_get::ClipboardGetArgs {
                format: a
                    .format
                    .as_deref()
                    .map(parse_clipboard_format)
                    .transpose()?,
                out: a.out,
            },
            adapter,
            context,
        ),
        Commands::ClipboardSet(a) => clipboard_set::execute(
            clipboard_set::ClipboardSetArgs {
                text: a.text,
                image: a.image,
                file_urls: a.file_url,
            },
            adapter,
        ),
        Commands::ClipboardClear => clipboard_clear::execute(adapter),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "clipboard::dispatch received a non-clipboard command",
            ),
        )),
    }
}
