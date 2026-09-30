use std::io::{self, Write};

use crate::scanner::Inventory;

/// Prevent repository-controlled text from issuing terminal commands or changing
/// the report's line structure, even when stdout is redirected.
pub fn terminal_text(text: &str) -> String {
    let mut safe = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control()
            || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            safe.extend(c.escape_default());
        } else {
            safe.push(c);
        }
    }
    safe
}

pub fn write_report(mut output: impl Write, inventory: &Inventory) -> io::Result<()> {
    writeln!(output, "Repository: {}", inventory.repository)?;
    match &inventory.commit {
        Some(commit) => writeln!(output, "Commit: {commit}")?,
        None => writeln!(output, "Commit: none (empty repository; no commit to scan)")?,
    }
    writeln!(output, "Skills found: {}", inventory.skills.len())?;
    if inventory.skills.is_empty() {
        writeln!(output, "No SKILL.md files found.")?;
    }
    for skill in &inventory.skills {
        writeln!(output, "\n{}", terminal_text(&skill.name))?;
        writeln!(
            output,
            "  Description: {}",
            terminal_text(&skill.description)
        )?;
        writeln!(output, "  Path: {}", terminal_text(&skill.path))?;
        writeln!(output, "  Link: {}", skill.link)?;
    }
    output.flush()
}

pub fn write_warnings(mut output: impl Write, inventory: &Inventory) -> io::Result<()> {
    for skill in &inventory.skills {
        for warning in &skill.warnings {
            writeln!(
                output,
                "warning: {}: {}: {}",
                terminal_text(&skill.path),
                warning.field,
                terminal_text(&warning.message)
            )?;
        }
    }
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{metadata::MetadataWarning, scanner::Skill};

    #[test]
    fn prints_empty_repository_without_inventing_a_commit() {
        let inventory = Inventory {
            repository: "a/b".parse().unwrap(),
            commit: None,
            skills: vec![],
        };
        let mut output = Vec::new();
        write_report(&mut output, &inventory).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "Repository: a/b\nCommit: none (empty repository; no commit to scan)\nSkills found: 0\nNo SKILL.md files found.\n");
    }

    #[test]
    fn keeps_warning_stream_separate_and_neutralizes_terminal_controls() {
        let inventory = Inventory {
            repository: "a/b".parse().unwrap(),
            commit: Some("abc".to_owned()),
            skills: vec![Skill {
                path: "skills/evil\n/SKILL.md".to_owned(),
                name: "name\x1b[2J".to_owned(),
                description: "Hello\u{202e}world".to_owned(),
                link: "https://github.com/a/b/blob/abc/skills/evil%0A/SKILL.md".to_owned(),
                warnings: vec![MetadataWarning {
                    field: "name",
                    message: "field is missing".to_owned(),
                }],
            }],
        };
        let mut output = Vec::new();
        let mut warnings = Vec::new();
        write_report(&mut output, &inventory).unwrap();
        write_warnings(&mut warnings, &inventory).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("name\\u{1b}[2J"));
        assert!(output.contains("Hello\\u{202e}world"));
        assert!(output.contains("skills/evil\\n/SKILL.md"));
        assert!(!output.contains("warning:"));
        assert_eq!(
            String::from_utf8(warnings).unwrap(),
            "warning: skills/evil\\n/SKILL.md: name: field is missing\n"
        );
    }
}
