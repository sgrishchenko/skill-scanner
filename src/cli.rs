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
    /// List SKILL.md files across an organization's or user's public repositories
    #[command(
        after_help = "Lists the owner's public repositories, skips forks, and scans each default branch in turn.\nRepositories that cannot be scanned are reported and the exit code is 1; the rest of the report is still printed.\nVery large repositories whose file listing GitHub truncates must be scanned individually.\nAnalyses share the scan cache; SKILL_SCANNER_CACHE_DIR overrides it and an empty value disables it.\nOrganization scans do not change recent repositories.\nSet GITHUB_TOKEN for larger organizations; anonymous access allows about 60 requests per hour.\nRepository content is read without running any scripts or skill instructions."
    )]
    ScanOrg {
        /// OWNER or https://github.com/OWNER (an organization or user)
        organization: String,
    },
    /// List or remove recently scanned repositories (no GitHub access)
    #[command(
        after_help = "Successful CLI and web scans share recent repositories across runs.\nSKILL_SCANNER_HISTORY_DIR overrides the history directory; an empty value disables it."
    )]
    Recent {
        #[command(subcommand)]
        action: Option<RecentAction>,
    },
    /// List or remove starred skills (no GitHub access)
    #[command(
        after_help = "Star skills from scan results in the web interface; see skill-scanner serve --help.\nStarred skills are shared by the CLI and web interface across runs.\nSKILL_SCANNER_STARRED_DIR overrides the starred skills directory; an empty value disables it."
    )]
    Starred {
        #[command(subcommand)]
        action: Option<StarredAction>,
    },
    /// Serve a local web interface at http://127.0.0.1:3000
    #[command(
        after_help = "Open the printed URL in your browser. Press Ctrl+C to stop.\nAnalyses share the CLI cache and are revalidated by commit on every scan.\nSKILL_SCANNER_CACHE_DIR overrides the cache directory; an empty value disables it.\nSuccessful scans are saved in recent repositories; see skill-scanner recent --help.\nStarred skills are saved on this machine; see skill-scanner starred --help.\nAn optional GITHUB_TOKEN raises GitHub's public API limits."
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

#[derive(Debug, Subcommand)]
pub enum StarredAction {
    /// Unstar a skill; keep its repository's history and cached analysis
    Remove {
        /// OWNER/REPO or https://github.com/OWNER/REPO
        repository: String,
        /// Repository-relative path of the skill's SKILL.md file
        path: String,
    },
}
