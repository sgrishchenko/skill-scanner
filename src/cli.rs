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
        after_help = "Scan progress and warnings are printed to stderr.\nAnalyses are cached locally and revalidated by commit on every scan.\nSKILL_SCANNER_CACHE_DIR overrides the cache directory; an empty value disables it.\nSuccessful scans are saved in recent repositories; see skill-scanner recent --help.\nAn optional GITHUB_TOKEN raises GitHub's public API limits.\nRepository content is read without running any scripts or skill instructions."
    )]
    Scan {
        /// OWNER/REPO or https://github.com/OWNER/REPO
        repository: String,
    },
    /// List or remove recently scanned repositories (no GitHub access)
    #[command(
        after_help = "Successful CLI and web scans share recent repositories across runs.\nSKILL_SCANNER_HISTORY_DIR overrides the history directory; an empty value disables it."
    )]
    Recent {
        #[command(subcommand)]
        action: Option<RecentAction>,
    },
    /// Serve a local web interface at http://127.0.0.1:3000
    #[command(
        after_help = "Open the printed URL in your browser. Press Ctrl+C to stop.\nAnalyses share the CLI cache and are revalidated by commit on every scan.\nSKILL_SCANNER_CACHE_DIR overrides the cache directory; an empty value disables it.\nSuccessful scans are saved in recent repositories; see skill-scanner recent --help.\nAn optional GITHUB_TOKEN raises GitHub's public API limits."
    )]
    Serve {
        /// Local port to listen on (use 0 to choose an available port)
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
pub enum RecentAction {
    /// Remove a repository from the recent list; keep its cached analysis
    Remove {
        /// OWNER/REPO or https://github.com/OWNER/REPO
        repository: String,
    },
}
