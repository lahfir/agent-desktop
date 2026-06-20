use clap::{Args, Subcommand};

#[derive(Args, Debug)]
#[command(after_help = "\
Skills travel inside the binary so they always match this exact
agent-desktop version. Output is raw markdown on stdout — parse it
directly, or redirect into a file for storage.

Examples:
  agent-desktop skills                        # List skills
  agent-desktop skills get desktop            # Primary guide
  agent-desktop skills get desktop --full     # Plus every reference
  agent-desktop skills get desktop workflows  # Single reference
  agent-desktop skills path                   # Where skills live")]
pub(crate) struct SkillsArgs {
    #[command(subcommand)]
    pub action: Option<SkillsAction>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SkillsAction {
    #[command(about = "List bundled skills with summaries (default)")]
    List,
    #[command(about = "Print a skill's markdown to stdout")]
    Get(SkillsGetArgs),
    #[command(about = "Print where bundled skills live")]
    Path,
}

#[derive(Args, Debug)]
pub(crate) struct SkillsGetArgs {
    #[arg(help = "Skill name or alias (desktop, ffi, ...)")]
    pub name: String,
    #[arg(
        help = "Reference filename (e.g. workflows or references/workflows.md). Omit for the main guide."
    )]
    pub reference: Option<String>,
    #[arg(long, help = "Append every reference file to the output")]
    pub full: bool,
}
