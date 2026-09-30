use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use clap::Parser;
use skill_scanner::{
    cache::ScanCache,
    cli::{Cli, Command, RecentAction},
    github::GitHubClient,
    recent::RecentRepositories,
    report,
    repository::Repository,
    scanner, web,
};

fn main() -> ExitCode {
    let Cli { command } = Cli::parse();
    // Parse separately so clap does not echo rejected URLs, which could contain
    // credentials, into diagnostics.
    let repository = match &command {
        Command::Scan { repository }
        | Command::Recent {
            action: Some(RecentAction::Remove { repository }),
        } => match repository.parse::<Repository>() {
            Ok(repository) => Some(repository),
            Err(error) => {
                let _ = writeln!(io::stderr().lock(), "error: {error}");
                return ExitCode::from(2);
            }
        },
        Command::Serve { .. } | Command::Recent { action: None } => None,
    };
    let recent = RecentRepositories::from_environment();
    if let Command::Recent { action } = command {
        let result = match action {
            Some(RecentAction::Remove { .. }) => {
                let repository = repository.expect("remove command has a repository");
                recent.remove(&repository).and_then(|()| {
                    writeln!(
                        io::stdout().lock(),
                        "Removed {repository} from recent repositories."
                    )
                })
            }
            None => recent.list().and_then(|entries| {
                report::write_recent(io::stdout().lock(), &entries, recent.is_enabled())
            }),
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(_) => {
                let _ = writeln!(io::stderr().lock(), "error: could not access recent repositories; check SKILL_SCANNER_HISTORY_DIR and directory permissions.");
                ExitCode::FAILURE
            }
        };
    }
    let token = match env::var("GITHUB_TOKEN") {
        Ok(token) if !token.trim().is_empty() => Some(token),
        Ok(_) | Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            let _ = writeln!(
                io::stderr().lock(),
                "error: GITHUB_TOKEN is not valid text; unset it or supply a valid token."
            );
            return ExitCode::FAILURE;
        }
    };
    let client = match GitHubClient::new(token.as_deref()) {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Command::Serve { port } = command {
        return match web::serve(port, client) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(
                    io::stderr().lock(),
                    "error: could not run the web interface: {error}"
                );
                ExitCode::FAILURE
            }
        };
    }
    let result = scanner::scan_with_storage(
        &client,
        &repository.expect("scan command has a repository"),
        &ScanCache::from_environment(),
        &recent,
        |progress| {
            let _ = report::write_progress(&mut io::stderr().lock(), progress);
        },
    );
    match result {
        Ok(inventory) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            if let Err(error) = report::write_warnings(&mut stderr, &inventory) {
                if error.kind() != io::ErrorKind::BrokenPipe {
                    return ExitCode::FAILURE;
                }
            }
            if let Err(error) = report::write_report(&mut io::stdout().lock(), &inventory) {
                if error.kind() != io::ErrorKind::BrokenPipe {
                    let _ = writeln!(stderr, "error: could not write the report: {error}");
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "error: {}",
                report::terminal_text(&error.to_string())
            );
            ExitCode::FAILURE
        }
    }
}
