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
        if owner.is_empty()
            || owner.len() > 39
            || !owner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || owner.starts_with('-')
            || owner.ends_with('-')
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
    fn encodes_links_without_interpreting_path_characters() {
        let repo: Repository = "a/b".parse().unwrap();
        assert_eq!(
            repo.file_url("abc", "a #?/é/SKILL.md"),
            "https://github.com/a/b/blob/abc/a%20%23%3F/%C3%A9/SKILL.md"
        );
    }
}
