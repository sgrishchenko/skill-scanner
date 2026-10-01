use std::{fmt, str::FromStr};

use reqwest::Url;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Repository {
    pub owner: String,
    pub name: String,
}

impl Repository {
    pub fn file_url(&self, commit: &str, path: &str) -> String {
        let mut url = Url::parse("https://github.com").expect("constant URL");
        url.path_segments_mut()
            .expect("HTTP URL supports path segments")
            .extend([self.owner.as_str(), self.name.as_str(), "blob", commit])
            .extend(path.split('/'));
        url.into()
    }
}

/// A GitHub organization or user account whose public repositories are scanned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub login: String,
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.login)
    }
}

impl FromStr for Owner {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let path = input.strip_prefix("https://github.com/").unwrap_or(input);
        let login = path.strip_suffix('/').unwrap_or(path);
        if !valid_owner(login) {
            return Err("expected OWNER or https://github.com/OWNER (one GitHub organization or user, without a repository, query, or fragment)".to_owned());
        }
        Ok(Self {
            login: login.to_owned(),
        })
    }
}

fn valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 39
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

impl FromStr for Repository {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            "expected OWNER/REPO or https://github.com/OWNER/REPO (one public repository, without a branch, query, or fragment)".to_owned()
        };
        // Parse the original path, rather than URL-normalized dot segments.
        let path = if let Some(path) = input.strip_prefix("https://github.com/") {
            path
        } else {
            input
        };
        let path = path.strip_suffix('/').unwrap_or(path);
        let (owner, name) = path.split_once('/').ok_or_else(invalid)?;
        let name = name.strip_suffix(".git").unwrap_or(name);
        if !valid_owner(owner)
            || name.is_empty()
            || name.len() > 100
            || name == "."
            || name == ".."
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(invalid());
        }
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_repository_identifiers_and_urls() {
        for input in [
            "example/skills",
            "example/skills/",
            "example/skills.git",
            "https://github.com/example/skills",
            "https://github.com/example/skills.git/",
        ] {
            assert_eq!(
                input.parse::<Repository>().unwrap().to_string(),
                "example/skills"
            );
        }
    }

    #[test]
    fn rejects_inputs_outside_one_public_github_repository() {
        for input in [
            "",
            "example",
            "../skills",
            "/example/skills",
            "a/..",
            "a/.",
            "a/b//",
            "a/b/tree/main",
            "a/b?token=secret",
            "a/b#main",
            " a/b",
            "https://gitlab.com/a/b",
            "http://github.com/a/b",
            "https://github.com:443/a/b",
            "https://user:secret@github.com/a/b",
            "https://github.com/a/../b",
            "https://github.com/a/b%2Ftree",
            "git@github.com:a/b.git",
            "-a/b",
            "a-/b",
            "a/b\\c",
        ] {
            assert!(input.parse::<Repository>().is_err(), "accepted {input}");
        }
    }

    #[test]
    fn accepts_owner_identifiers_and_urls_only() {
        for input in [
            "JetBrains",
            "JetBrains/",
            "https://github.com/JetBrains",
            "https://github.com/JetBrains/",
        ] {
            assert_eq!(input.parse::<Owner>().unwrap().to_string(), "JetBrains");
        }
        let long = "a".repeat(40);
        for input in [
            "",
            "/",
            "JetBrains/skills",
            "https://github.com/JetBrains/skills",
            "https://github.com/orgs/JetBrains",
            "JetBrains//",
            " JetBrains",
            "-JetBrains",
            "JetBrains-",
            "Jet_Brains",
            "JetBrains?type=all",
            "JetBrains#repos",
            "http://github.com/JetBrains",
            "https://github.com:443/JetBrains",
            "https://user:secret@github.com/JetBrains",
            "https://github.com/..",
            long.as_str(),
        ] {
            assert!(input.parse::<Owner>().is_err(), "accepted {input}");
        }
    }

    #[test]
    fn encodes_links_without_interpreting_path_characters() {
        let repo: Repository = "a/b".parse().unwrap();
        assert_eq!(
            repo.file_url("abc", "a #?/é/SKILL.md"),
            "https://github.com/a/b/blob/abc/a%20%23%3F/%C3%A9/SKILL.md"
        );
    }
}
