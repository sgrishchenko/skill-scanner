use std::io::{self, Write};

use crate::{
    aggregation,
    scanner::{Inventory, ScanProgress},
};

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

pub fn write_progress(mut output: impl Write, progress: ScanProgress<'_>) -> io::Result<()> {
    match progress {
        ScanProgress::Repository(repository) => {
            writeln!(output, "Resolving repository: {repository}")?;
        }
        ScanProgress::DefaultBranch(branch) => {
            writeln!(
                output,
                "Resolving default branch: {}",
                terminal_text(branch)
            )?;
        }
        ScanProgress::Cached(commit) => {
            writeln!(
                output,
                "Using cached analysis for commit: {}",
                terminal_text(commit)
            )?;
        }
        ScanProgress::DiscoveringSkills => {
            writeln!(output, "Discovering SKILL.md files: /")?;
        }
        ScanProgress::Directory(path) => {
            writeln!(output, "Scanning directory: {}", terminal_text(path))?;
        }
        ScanProgress::Skill {
            path,
            current,
            total,
        } => {
            writeln!(
                output,
                "Scanning skill [{current}/{total}]: {}",
                terminal_text(path)
            )?;
        }
    }
    output.flush()
}

pub fn write_report(mut output: impl Write, inventory: &Inventory) -> io::Result<()> {
    writeln!(output, "Repository: {}", inventory.repository)?;
    match &inventory.commit {
        Some(commit) => writeln!(output, "Commit: {commit}")?,
        None => writeln!(output, "Commit: none (empty repository; no commit to scan)")?,
    }
    writeln!(output, "Skills found: {}", inventory.skills.len())?;
    let aggregation = aggregation::aggregate(&inventory.skills);
    let stats = &aggregation.statistics;
    let percentage = |count: usize| {
        if stats.total_skills == 0 {
            0.0
        } else {
            100.0 * count as f64 / stats.total_skills as f64
        }
    };
    writeln!(output, "Similar groups: {}", stats.similar_groups)?;
    writeln!(
        output,
        "Skills in similar groups: {} ({:.1}%)",
        stats.grouped_skills,
        percentage(stats.grouped_skills)
    )?;
    writeln!(output, "Standalone skills: {}", stats.standalone_skills)?;
    writeln!(output, "Largest similar group: {}", stats.largest_group)?;
    writeln!(
        output,
        "Skills with warnings: {}",
        stats.skills_with_warnings
    )?;
    if inventory.skills.is_empty() {
        writeln!(output, "No SKILL.md files found.")?;
    }
    if stats.similar_groups > 0 {
        writeln!(
            output,
            "\nSimilar skills (matching names or descriptions, ignoring case and punctuation):"
        )?;
        writeln!(output, "Groups include skills connected through other members; only valid metadata is compared.")?;
        for group in aggregation
            .groups
            .iter()
            .filter(|group| group.skill_indices.len() > 1)
        {
            let size = group.skill_indices.len();
            let name = &inventory.skills[group.skill_indices[0]].name;
            writeln!(
                output,
                "\n  {}: {size} skills ({:.1}% of scan), {} with warnings",
                terminal_text(name),
                percentage(size),
                group.skills_with_warnings
            )?;
            for &index in &group.skill_indices {
                writeln!(
                    output,
                    "    {}",
                    terminal_text(&inventory.skills[index].path)
                )?;
            }
        }
        writeln!(output, "\nAll skills (sorted by path):")?;
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
        assert_eq!(String::from_utf8(output).unwrap(), "Repository: a/b\nCommit: none (empty repository; no commit to scan)\nSkills found: 0\nSimilar groups: 0\nSkills in similar groups: 0 (0.0%)\nStandalone skills: 0\nLargest similar group: 0\nSkills with warnings: 0\nNo SKILL.md files found.\n");
    }

    #[test]
    fn reports_group_statistics_and_keeps_every_source() {
        let inventory = Inventory {
            repository: "a/b".parse().unwrap(),
            commit: Some("abc".to_owned()),
            skills: ["code-review\n", "Code Review", "release-notes"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| Skill {
                    path: format!("{index}/SKILL.md"),
                    name: name.to_owned(),
                    description: format!("Description {index}"),
                    link: format!("https://github.com/a/b/blob/abc/{index}/SKILL.md"),
                    warnings: if index == 0 {
                        vec![MetadataWarning {
                            field: "description".to_owned(),
                            message: "field is missing".to_owned(),
                        }]
                    } else {
                        vec![]
                    },
                })
                .collect(),
        };
        let mut output = Vec::new();
        write_report(&mut output, &inventory).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Similar groups: 1\nSkills in similar groups: 2 (66.7%)\nStandalone skills: 1\nLargest similar group: 2\nSkills with warnings: 1\n"));
        assert!(output.contains("  code-review\\n: 2 skills (66.7% of scan), 1 with warnings\n    0/SKILL.md\n    1/SKILL.md"));
        assert_eq!(output.matches("  Link: https://github.com/").count(), 3);
        assert!(output.find("  Path: 0/").unwrap() < output.find("  Path: 2/").unwrap());
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
                    field: "name".to_owned(),
                    message: "field is missing".to_owned(),
                }],
            }],
        };
        let mut output = Vec::new();
        let mut warnings = Vec::new();
        let mut progress = Vec::new();
        write_report(&mut output, &inventory).unwrap();
        write_warnings(&mut warnings, &inventory).unwrap();
        write_progress(&mut progress, ScanProgress::DefaultBranch("main\x1b[2J")).unwrap();
        write_progress(&mut progress, ScanProgress::Directory("skills/evil\n")).unwrap();
        write_progress(
            &mut progress,
            ScanProgress::Skill {
                path: &inventory.skills[0].path,
                current: 1,
                total: 1,
            },
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("name\\u{1b}[2J"));
        assert!(output.contains("Hello\\u{202e}world"));
        assert!(output.contains("skills/evil\\n/SKILL.md"));
        assert!(!output.contains("warning:"));
        assert_eq!(
            String::from_utf8(warnings).unwrap(),
            "warning: skills/evil\\n/SKILL.md: name: field is missing\n"
        );
        assert_eq!(
            String::from_utf8(progress).unwrap(),
            "Resolving default branch: main\\u{1b}[2J\nScanning directory: skills/evil\\n\nScanning skill [1/1]: skills/evil\\n/SKILL.md\n"
        );
    }
}
