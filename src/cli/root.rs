use std::path::PathBuf;

use clap::Parser;

use super::{Commands, post_action_wait::PostActionWaitArgs};

const BEFORE_HELP: &str = include_str!("help_before.txt");
const AFTER_HELP: &str = include_str!("help_after.txt");

#[derive(Parser, Debug)]
#[command(
    name = "agent-desktop",
    version,
    about = "Desktop automation CLI for AI agents",
    long_about = None,
    before_help = BEFORE_HELP,
    after_help = AFTER_HELP,
)]
pub(crate) struct Cli {
    #[arg(
        long,
        short = 'v',
        global = true,
        help = "Enable debug logging to stderr"
    )]
    pub verbose: bool,
    #[arg(
        long,
        global = true,
        help = "Select the snapshot namespace; session-owned refs require the same scope"
    )]
    pub session: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Append reliability trace JSONL to this path"
    )]
    pub trace: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Fail on trace setup/pre-action write errors"
    )]
    pub trace_strict: bool,
    #[arg(
        long,
        global = true,
        help = "Prefer physical delivery for natural input commands and permit focus/cursor side effects. Default is strict headless semantic delivery."
    )]
    pub headed: bool,
    #[command(flatten)]
    pub post_action_wait: PostActionWaitArgs,
    #[command(subcommand)]
    pub command: Option<Commands>,
}
