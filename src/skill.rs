use std::path::Path;

#[derive(Debug)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
}

pub fn parse_frontmatter(content: &str) -> Result<SkillFrontmatter, String> {
    let trimmed = content.trim();

    // Must start with exactly "---" followed by a newline
    if !trimmed.starts_with("---") {
        return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
    }

    // Find the closing --- on its own line
    let after_open = &trimmed[3..];
    let after_open = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))
        .ok_or("SKILL.md frontmatter: expected newline after opening ---")?;

    let close = find_closing_fence(after_open).ok_or("SKILL.md frontmatter missing closing ---")?;
    let frontmatter = &after_open[..close];

    let name = extract_field(frontmatter, "name")
        .ok_or("SKILL.md missing required 'name' field in frontmatter")?;
    let description = extract_field(frontmatter, "description")
        .ok_or("SKILL.md missing required 'description' field in frontmatter")?;
    let version = extract_field(frontmatter, "version");

    Ok(SkillFrontmatter {
        name,
        description,
        version,
    })
}

/// Find closing `---` that appears at the start of a line
fn find_closing_fence(content: &str) -> Option<usize> {
    for (i, line) in content.lines().enumerate() {
        if line.trim() == "---" {
            // Calculate byte offset
            let offset: usize = content.lines().take(i).map(|l| l.len() + 1).sum();
            return Some(offset);
        }
    }
    None
}

fn extract_field(frontmatter: &str, field: &str) -> Option<String> {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        // Match "field:" or "field :" exactly — the key must be the complete word
        if let Some(rest) = line.strip_prefix(field) {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix(':') {
                let value = value.trim();

                // YAML folded (>) or literal (|) multiline scalar
                if value == ">" || value == "|" {
                    let folded = value == ">";
                    let mut parts = Vec::new();
                    i += 1;
                    while i < lines.len() {
                        let next = lines[i];
                        // Continuation lines must be indented
                        if next.starts_with(' ') || next.starts_with('\t') {
                            parts.push(next.trim());
                        } else {
                            break;
                        }
                        i += 1;
                    }
                    let joined = if folded {
                        parts.join(" ")
                    } else {
                        parts.join("\n")
                    };
                    if !joined.is_empty() {
                        return Some(joined);
                    }
                    return None;
                }

                // Strip surrounding quotes if present
                let value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value);
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        i += 1;
    }
    None
}

pub fn read_skill(skill_dir: &Path) -> Result<SkillFrontmatter, String> {
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err(format!("No SKILL.md found in {}", skill_dir.display()));
    }
    let content = std::fs::read_to_string(&skill_md)
        .map_err(|e| format!("Failed to read {}: {}", skill_md.display(), e))?;
    parse_frontmatter(&content)
}

/// Scan a directory for skills. Returns (skill_dir_path, frontmatter) pairs.
pub fn discover_skills(dir: &Path) -> Result<Vec<(std::path::PathBuf, SkillFrontmatter)>, String> {
    let skill_md = dir.join("SKILL.md");
    if skill_md.exists() {
        let fm = read_skill(dir)?;
        return Ok(vec![(dir.to_path_buf(), fm)]);
    }

    let mut skills = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            match read_skill(&path) {
                Ok(fm) => skills.push((path, fm)),
                Err(e) => eprintln!("Warning: skipping {}: {e}", path.display()),
            }
        }
    }

    if skills.is_empty() {
        return Err(format!("No SKILL.md files found in {}", dir.display()));
    }

    skills.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    Ok(skills)
}

/// Read an includes file: one source per line, # comments, blank lines ignored
pub fn read_includes(path: &Path) -> Result<Vec<String>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read includes: {e}"))?;
    Ok(content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_frontmatter() {
        let content = "---\nname: my-skill\ndescription: A test skill\n---\n# Hello";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "my-skill");
        assert_eq!(fm.description, "A test skill");
    }

    #[test]
    fn parse_quoted_values() {
        let content = "---\nname: \"my-skill\"\ndescription: 'A test skill'\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "my-skill");
        assert_eq!(fm.description, "A test skill");
    }

    #[test]
    fn missing_name() {
        let content = "---\ndescription: A test skill\n---\n";
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn missing_description() {
        let content = "---\nname: my-skill\n---\n";
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("description"));
    }

    #[test]
    fn no_frontmatter() {
        let content = "# Just a markdown file";
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("frontmatter"));
    }

    #[test]
    fn no_closing_dashes() {
        let content = "---\nname: test\ndescription: test\n";
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("closing"));
    }

    #[test]
    fn four_dashes_rejected() {
        let content = "----\nname: test\ndescription: test\n---\n";
        // Should require newline immediately after "---"
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("newline"));
    }

    #[test]
    fn parse_version_field() {
        let content = "---\nname: my-skill\ndescription: A test\nversion: 1.2.0\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.version, Some("1.2.0".to_string()));
    }

    #[test]
    fn parse_no_version_field() {
        let content = "---\nname: my-skill\ndescription: A test\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.version, None);
    }

    #[test]
    fn dashes_in_body_dont_close_frontmatter() {
        let content =
            "---\nname: my-skill\ndescription: A test skill\n---\n# Body\n\n--- separator ---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "my-skill");
        assert_eq!(fm.description, "A test skill");
    }

    #[test]
    fn parse_folded_description() {
        let content = "---\nname: available\ndescription: >\n  Find and check project name\n  availability across registries.\nuser-invocable: true\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(
            fm.description,
            "Find and check project name availability across registries."
        );
    }

    #[test]
    fn parse_literal_description() {
        let content = "---\nname: my-skill\ndescription: |\n  Line one.\n  Line two.\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.description, "Line one.\nLine two.");
    }
}
