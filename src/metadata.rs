use serde_yaml_ng::{Mapping, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataWarning {
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct Metadata {
    pub name: String,
    pub description: String,
    pub warnings: Vec<MetadataWarning>,
}

impl Metadata {
    pub fn unavailable(fallback: &str, reason: &str) -> Self {
        Self {
            name: fallback.to_owned(),
            description: String::new(),
            warnings: ["name", "description"]
                .into_iter()
                .map(|field| MetadataWarning {
                    field,
                    message: reason.to_owned(),
                })
                .collect(),
        }
    }
}

pub fn parse(bytes: &[u8], fallback: &str) -> Metadata {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Metadata::unavailable(fallback, "file is not valid UTF-8");
    };
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Metadata::unavailable(fallback, "YAML front matter is missing");
    }
    let mut yaml = String::new();
    let mut closed = false;
    for line in lines {
        if matches!(line.trim_end(), "---" | "...") {
            closed = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    if !closed {
        return Metadata::unavailable(fallback, "YAML front matter has no closing delimiter");
    }
    let mapping = match serde_yaml_ng::from_str::<Value>(&yaml) {
        Ok(Value::Mapping(mapping)) => mapping,
        Ok(Value::Null) => Mapping::new(),
        Ok(_) => return Metadata::unavailable(fallback, "YAML front matter must be a mapping"),
        Err(_) => return Metadata::unavailable(fallback, "YAML front matter is malformed"),
    };
    let mut warnings = Vec::new();
    let name = field(&mapping, "name", &mut warnings).unwrap_or_else(|| fallback.to_owned());
    let description = field(&mapping, "description", &mut warnings).unwrap_or_default();
    Metadata {
        name,
        description,
        warnings,
    }
}

fn field(
    mapping: &Mapping,
    key: &'static str,
    warnings: &mut Vec<MetadataWarning>,
) -> Option<String> {
    let reason = match mapping.get(Value::String(key.to_owned())) {
        Some(Value::String(value)) if !value.trim().is_empty() => {
            return Some(value.trim().to_owned())
        }
        Some(Value::String(_)) => "field must not be empty",
        Some(_) => "field must be a string",
        None => "field is missing",
    };
    warnings.push(MetadataWarning {
        field: key,
        message: reason.to_owned(),
    });
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bom_crlf_and_multiline_yaml_without_reading_instructions() {
        let metadata = parse(b"\xef\xbb\xbf---\r\nname: review\r\ndescription: >-\r\n  Review changes\r\n  for correctness.\r\nextra: ignored\r\n---\r\nrun: rm -rf /\n", "fallback");
        assert_eq!(metadata.name, "review");
        assert_eq!(metadata.description, "Review changes for correctness.");
        assert!(metadata.warnings.is_empty());
    }

    #[test]
    fn reports_each_missing_or_invalid_field_without_losing_valid_fields() {
        let metadata = parse(
            b"---\nname: 12\ndescription: Still useful\n---",
            "directory",
        );
        assert_eq!(metadata.name, "directory");
        assert_eq!(metadata.description, "Still useful");
        assert_eq!(metadata.warnings.len(), 1);
        assert_eq!(metadata.warnings[0].field, "name");
        let metadata = parse(b"---\nname: okay\ndescription: '  '\n...", "directory");
        assert_eq!(metadata.name, "okay");
        assert_eq!(metadata.warnings[0].field, "description");
    }

    #[test]
    fn keeps_discovery_when_front_matter_is_unreadable() {
        for bytes in [
            &b"plain markdown"[..],
            &b"---\nname: unclosed"[..],
            &b"---\nname: [\n---"[..],
            &b"---\n- a list\n---"[..],
            &b"---\nname: one\nname: two\n---"[..],
            &[0xff][..],
            &b"---\n---"[..],
        ] {
            let metadata = parse(bytes, "fallback");
            assert_eq!(metadata.name, "fallback");
            assert!(metadata.description.is_empty());
            assert_eq!(metadata.warnings.len(), 2);
        }
    }
}
