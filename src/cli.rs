use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Discover AI skills in one public GitHub repository")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List SKILL.md files on the repository's default branch
    #[command(
        after_help = "Scan progress and warnings are printed to stderr.\nAn optional GITHUB_TOKEN raises GitHub's public API limits.\nRepository content is read without running any scripts or skill instructions."
    )]
    Scan {
        /// OWNER/REPO or https://github.com/OWNER/REPO
        repository: String,
    },
}
