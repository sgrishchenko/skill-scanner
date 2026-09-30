use std::collections::HashMap;

use crate::scanner::Skill;

#[derive(Debug, serde::Serialize)]
pub struct Aggregation {
    pub statistics: Statistics,
    /// Every skill belongs to exactly one group, including standalone skills.
    pub groups: Vec<SkillGroup>,
}

#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Statistics {
    pub total_skills: usize,
    pub similar_groups: usize,
    pub grouped_skills: usize,
    pub standalone_skills: usize,
    /// Zero when there are no groups containing at least two skills.
    pub largest_group: usize,
    pub skills_with_warnings: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct SkillGroup {
    /// Indices into the original inventory, ordered by repository path.
    pub skill_indices: Vec<usize>,
    pub skills_with_warnings: usize,
}

/// Group matching valid names or descriptions, including transitive matches.
/// Use indexed lookups instead of comparing every pair of skills. Missing
/// metadata and fallback names are never evidence of similarity.
pub fn aggregate(skills: &[Skill]) -> Aggregation {
    let mut parents: Vec<usize> = (0..skills.len()).collect();
    let mut names = HashMap::new();
    let mut descriptions = HashMap::new();
    for (index, skill) in skills.iter().enumerate() {
        for (field, value, lookup) in [
            ("name", &skill.name, &mut names),
            ("description", &skill.description, &mut descriptions),
        ] {
            if skill.warnings.iter().any(|warning| warning.field == field) {
                continue;
            }
            let key = normalized(value);
            if key.is_empty() {
                continue;
            }
            if let Some(previous) = lookup.insert(key, index) {
                let root = root(&mut parents, previous);
                parents[root] = index;
            }
        }
    }

    let mut members = HashMap::<usize, Vec<usize>>::new();
    for index in 0..skills.len() {
        members
            .entry(root(&mut parents, index))
            .or_default()
            .push(index);
    }
    let mut groups: Vec<SkillGroup> = members
        .into_values()
        .map(|mut skill_indices| {
            skill_indices.sort_by(|&left, &right| skills[left].path.cmp(&skills[right].path));
            let skills_with_warnings = skill_indices
                .iter()
                .filter(|&&index| !skills[index].warnings.is_empty())
                .count();
            SkillGroup {
                skill_indices,
                skills_with_warnings,
            }
        })
        .collect();
    groups.sort_by(|left, right| {
        right
            .skill_indices
            .len()
            .cmp(&left.skill_indices.len())
            .then_with(|| {
                skills[left.skill_indices[0]]
                    .path
                    .cmp(&skills[right.skill_indices[0]].path)
            })
    });

    let mut statistics = Statistics {
        total_skills: skills.len(),
        ..Statistics::default()
    };
    for group in &groups {
        statistics.skills_with_warnings += group.skills_with_warnings;
        let size = group.skill_indices.len();
        if size > 1 {
            statistics.similar_groups += 1;
            statistics.grouped_skills += size;
            statistics.largest_group = statistics.largest_group.max(size);
        } else {
            statistics.standalone_skills += 1;
        }
    }
    Aggregation { statistics, groups }
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    // Iterative path compression also handles long chains of matching metadata.
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn normalized(value: &str) -> String {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataWarning;

    fn skill(path: &str, name: &str, description: &str, invalid: &[&'static str]) -> Skill {
        Skill {
            path: path.to_owned(),
            name: name.to_owned(),
            description: description.to_owned(),
            link: String::new(),
            warnings: invalid
                .iter()
                .map(|&field| MetadataWarning {
                    field: field.to_owned(),
                    message: "field is missing".to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn merges_overlapping_matches_once_and_counts_skills_not_warnings() {
        let skills = vec![
            skill("d/SKILL.md", "Code Review", "Review pull requests.", &[]),
            skill("b/SKILL.md", "code_review", "Inspect changes", &[]),
            skill("a/SKILL.md", "another-name", "REVIEW  pull requests!", &[]),
            skill("c/SKILL.md", "release-notes", "Draft release notes", &[]),
            skill("e/SKILL.md", "code-review", "", &["description"]),
            skill("f/SKILL.md", "unavailable", "", &["name", "description"]),
        ];
        let result = aggregate(&skills);
        assert_eq!(result.groups[0].skill_indices, [2, 1, 0, 4]);
        assert_eq!(result.groups[0].skills_with_warnings, 1);
        assert_eq!(result.groups[1].skill_indices, [3]);
        assert_eq!(result.groups[2].skill_indices, [5]);
        assert_eq!(
            result.statistics,
            Statistics {
                total_skills: 6,
                similar_groups: 1,
                grouped_skills: 4,
                standalone_skills: 2,
                largest_group: 4,
                skills_with_warnings: 2,
            }
        );
    }

    #[test]
    fn ignores_missing_metadata_and_does_not_compare_names_to_descriptions() {
        let skills = vec![
            skill("a", "fallback", "", &["name", "description"]),
            skill("b", "fallback", "", &["name", "description"]),
            skill("c", "!!!", "...", &[]),
            skill("d", "???", "---", &[]),
            skill("e", "review", "notes", &[]),
            skill("f", "notes", "review", &[]),
            skill("g", "re view", "other", &[]),
        ];
        let result = aggregate(&skills);
        assert_eq!(result.statistics.similar_groups, 0);
        assert_eq!(result.statistics.largest_group, 0);
        assert_eq!(result.statistics.standalone_skills, skills.len());
    }

    #[test]
    fn groups_unicode_metadata_and_orders_equal_sizes_by_path() {
        let skills = vec![
            skill("z", "ÉCRIRE—NOTES", "", &[]),
            skill("y", "écrire notes", "", &[]),
            skill("b", "审查", "", &[]),
            skill("a", "审查", "", &[]),
        ];
        let result = aggregate(&skills);
        assert_eq!(result.statistics.similar_groups, 2);
        assert_eq!(result.groups[0].skill_indices, [3, 2]);
        assert_eq!(result.groups[1].skill_indices, [1, 0]);
        let reversed: Vec<_> = skills.into_iter().rev().collect();
        let reversed_result = aggregate(&reversed);
        assert_eq!(
            reversed[reversed_result.groups[0].skill_indices[0]].path,
            "a"
        );
        assert_eq!(
            reversed[reversed_result.groups[1].skill_indices[0]].path,
            "y"
        );
    }

    #[test]
    fn empty_inventory_has_zero_statistics() {
        let result = aggregate(&[]);
        assert_eq!(result.statistics, Statistics::default());
        assert!(result.groups.is_empty());
    }

    #[test]
    fn handles_large_groups_without_recursive_traversal() {
        let skills: Vec<_> = (0..10_000)
            .map(|index| skill(&format!("{index:05}"), "review", "Review changes", &[]))
            .collect();
        let result = aggregate(&skills);
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.statistics.grouped_skills, skills.len());
        assert_eq!(
            result.groups[0].skill_indices,
            (0..skills.len()).collect::<Vec<_>>()
        );
    }
}
