use std::path::PathBuf;

#[derive(Debug)]
pub enum SkillSource {
    GitHub {
        owner: String,
        repo: String,
        subpath: Option<String>,
    },
    GitUrl {
        url: String,
    },
    Local {
        path: PathBuf,
    },
}

/// Normalize messy input into something parseable.
/// Handles GitHub URLs, pasted commands from other tools, trailing noise.
fn normalize_input(source: &str) -> String {
    let source = source.trim();

    // Extract GitHub owner/repo from full URLs:
    //   https://github.com/owner/repo/blob/main/SKILL.md → owner/repo
    //   https://github.com/owner/repo/tree/main/skills/foo → owner/repo/skills/foo
    //   https://github.com/owner/repo → owner/repo
    //   https://github.com/owner/repo.git → owner/repo
    if let Some(path) = source
        .strip_prefix("https://github.com/")
        .or_else(|| source.strip_prefix("http://github.com/"))
    {
        let path = path.trim_end_matches('/');
        let parts: Vec<&str> = path.splitn(4, '/').collect();
        if parts.len() >= 2 {
            let owner = parts[0];
            let repo = parts[1].trim_end_matches(".git");

            if parts.len() <= 2 {
                return format!("{owner}/{repo}");
            }

            // parts[2] is "blob", "tree", "commit", "raw", etc.
            let segment = parts[2];
            if matches!(segment, "blob" | "tree" | "raw" | "commit") {
                // parts[3] is "main/path/to/thing" — strip the branch ref
                if let Some(rest) = parts.get(3) {
                    // Strip branch name (everything up to first /)
                    if let Some(after_branch) = rest.split_once('/').map(|(_, p)| p) {
                        // Strip file names like SKILL.md, README.md
                        let after_branch = strip_trailing_files(after_branch);
                        if after_branch.is_empty() {
                            return format!("{owner}/{repo}");
                        }
                        return format!("{owner}/{repo}/{after_branch}");
                    }
                }
                return format!("{owner}/{repo}");
            }

            // Not a known GitHub path segment — treat as subpath
            let joined = parts[2..].join("/");
            let subpath = strip_trailing_files(&joined);
            if subpath.is_empty() {
                return format!("{owner}/{repo}");
            }
            return format!("{owner}/{repo}/{subpath}");
        }
    }

    // Extract GitHub URLs/shorthand from pasted commands:
    //   "npx skills add https://github.com/user/repo --skill foo" → recurse on the URL
    //   "equip install owner/repo" → owner/repo
    if source.contains("github.com/") {
        for word in source.split_whitespace() {
            if word.contains("github.com/") {
                return normalize_input(word);
            }
        }
    }

    // If it looks like a pasted command with a shorthand, extract the owner/repo part
    // e.g. "npx skills add owner/repo --skill foo"
    for word in source.split_whitespace() {
        // owner/repo pattern: contains exactly 1+ slashes, no protocol prefix
        if word.contains('/')
            && !word.starts_with('-')
            && !word.starts_with("http")
            && !word.starts_with("git")
            && !word.starts_with('/')
            && !word.starts_with('.')
        {
            return word.to_string();
        }
    }

    source.to_string()
}

/// Strip trailing file names that aren't useful as subpaths
fn strip_trailing_files(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    if let Some((parent, file)) = path.rsplit_once('/') {
        if file.contains('.') {
            return parent;
        }
    } else if path.contains('.') {
        return "";
    }
    path
}

impl SkillSource {
    pub fn parse(source: &str) -> Result<Self, String> {
        // Normalize: extract useful parts from messy input
        let source = &normalize_input(source);

        // Local path: starts with /, ./, ../, ~, or Windows drive letter (C:\)
        if source.starts_with('/')
            || source.starts_with("./")
            || source.starts_with("../")
            || source.starts_with('~')
            || source == "."
            || (source.len() >= 3
                && source.as_bytes()[1] == b':'
                && (source.as_bytes()[2] == b'\\' || source.as_bytes()[2] == b'/'))
        {
            let path = if let Some(rest) = source.strip_prefix('~') {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .map_err(|_| {
                        "Could not determine home directory (HOME or USERPROFILE not set)"
                            .to_string()
                    })?;
                PathBuf::from(home).join(
                    rest.strip_prefix('/')
                        .or_else(|| rest.strip_prefix('\\'))
                        .unwrap_or(rest),
                )
            } else {
                PathBuf::from(source)
                    .canonicalize()
                    .map_err(|e| format!("Invalid path '{}': {}", source, e))?
            };
            return Ok(SkillSource::Local { path });
        }

        // Git URL: git@ or git:// (not GitHub HTTPS — those are normalized above)
        if source.starts_with("git@") || source.starts_with("git://") || source.ends_with(".git") {
            return Ok(SkillSource::GitUrl {
                url: source.to_string(),
            });
        }

        // GitHub shorthand: owner/repo or owner/repo/subpath
        let parts: Vec<&str> = source.splitn(3, '/').collect();
        match parts.len() {
            2 | 3 => {
                let owner = parts[0];
                let repo = parts[1];
                if owner.is_empty() || repo.is_empty() {
                    return Err(format!(
                        "Invalid source '{}': owner and repo must not be empty.",
                        source
                    ));
                }
                let subpath = if parts.len() == 3 {
                    let sp = parts[2];
                    if sp.is_empty() {
                        None
                    } else {
                        Some(sp.to_string())
                    }
                } else {
                    None
                };
                Ok(SkillSource::GitHub {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    subpath,
                })
            }
            _ => Err(format!(
                "Invalid source '{}'. Expected: owner/repo, a git URL, or a local path.",
                source
            )),
        }
    }

    pub fn repo_url(&self) -> Option<String> {
        match self {
            SkillSource::GitHub { owner, repo, .. } => {
                Some(format!("https://github.com/{}/{}", owner, repo))
            }
            SkillSource::GitUrl { url } => Some(url.clone()),
            SkillSource::Local { .. } => None,
        }
    }

    pub fn git_clone_url(&self) -> Option<String> {
        match self {
            SkillSource::GitHub { owner, repo, .. } => {
                Some(format!("https://github.com/{}/{}.git", owner, repo))
            }
            SkillSource::GitUrl { url } => Some(url.clone()),
            SkillSource::Local { .. } => None,
        }
    }

    pub fn subpath(&self) -> Option<&str> {
        match self {
            SkillSource::GitHub { subpath, .. } => subpath.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_shorthand() {
        let source = SkillSource::parse("anthropics/skills").unwrap();
        match source {
            SkillSource::GitHub {
                owner,
                repo,
                subpath,
            } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert!(subpath.is_none());
            }
            _ => panic!("Expected GitHub source"),
        }
    }

    #[test]
    fn parse_github_with_subpath() {
        let source = SkillSource::parse("anthropics/skills/pdf").unwrap();
        match source {
            SkillSource::GitHub {
                owner,
                repo,
                subpath,
            } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath.as_deref(), Some("pdf"));
            }
            _ => panic!("Expected GitHub source"),
        }
    }

    #[test]
    fn parse_github_https_url() {
        // GitHub HTTPS URLs are normalized to GitHub shorthand
        let source = SkillSource::parse("https://github.com/foo/bar.git").unwrap();
        match source {
            SkillSource::GitHub { owner, repo, .. } => {
                assert_eq!(owner, "foo");
                assert_eq!(repo, "bar");
            }
            _ => panic!("Expected GitHub source, got {:?}", source),
        }
    }

    #[test]
    fn parse_git_url_ssh() {
        let source = SkillSource::parse("git@github.com:foo/bar.git").unwrap();
        match source {
            SkillSource::GitUrl { url } => {
                assert_eq!(url, "git@github.com:foo/bar.git");
            }
            _ => panic!("Expected GitUrl source"),
        }
    }

    #[test]
    fn parse_local_relative() {
        // Can't easily test canonicalize without a real path, so test the detection
        let source = SkillSource::parse("./my-skill");
        // Will fail due to path not existing, but should try Local path
        assert!(source.is_err()); // canonicalize fails on non-existent path
    }

    #[test]
    fn parse_local_absolute() {
        let tmp = std::env::temp_dir();
        let source = SkillSource::parse(tmp.to_str().unwrap()).unwrap();
        match source {
            SkillSource::Local { path } => {
                assert!(std::path::Path::new(&path).is_absolute());
            }
            _ => panic!("Expected Local source"),
        }
    }

    #[test]
    fn repo_url_github() {
        let source = SkillSource::GitHub {
            owner: "anthropics".to_string(),
            repo: "skills".to_string(),
            subpath: None,
        };
        assert_eq!(
            source.repo_url(),
            Some("https://github.com/anthropics/skills".to_string())
        );
    }

    // --- normalize_input tests ---

    #[test]
    fn normalize_github_blob_url() {
        assert_eq!(
            normalize_input("https://github.com/michaelneale/megamind/blob/main/SKILL.md"),
            "michaelneale/megamind"
        );
    }

    #[test]
    fn normalize_github_tree_url_with_subpath() {
        assert_eq!(
            normalize_input(
                "https://github.com/anthropics/skills/tree/main/skills/frontend-design"
            ),
            "anthropics/skills/skills/frontend-design"
        );
    }

    #[test]
    fn normalize_github_plain_url() {
        assert_eq!(
            normalize_input("https://github.com/owner/repo"),
            "owner/repo"
        );
    }

    #[test]
    fn normalize_github_url_with_dot_git() {
        assert_eq!(
            normalize_input("https://github.com/owner/repo.git"),
            "owner/repo"
        );
    }

    #[test]
    fn normalize_github_url_trailing_slash() {
        assert_eq!(
            normalize_input("https://github.com/owner/repo/"),
            "owner/repo"
        );
    }

    #[test]
    fn normalize_pasted_npx_command() {
        assert_eq!(
            normalize_input(
                "npx skills add https://github.com/vercel-labs/agent-skills --skill web-design"
            ),
            "vercel-labs/agent-skills"
        );
    }

    #[test]
    fn normalize_pasted_equip_command() {
        assert_eq!(normalize_input("equip install owner/repo"), "owner/repo");
    }

    #[test]
    fn normalize_shorthand_passthrough() {
        assert_eq!(normalize_input("owner/repo"), "owner/repo");
        assert_eq!(normalize_input("owner/repo/subpath"), "owner/repo/subpath");
    }

    #[test]
    fn normalize_github_blob_url_resolves_to_github_source() {
        let source =
            SkillSource::parse("https://github.com/michaelneale/megamind/blob/main/SKILL.md")
                .unwrap();
        match source {
            SkillSource::GitHub { owner, repo, .. } => {
                assert_eq!(owner, "michaelneale");
                assert_eq!(repo, "megamind");
            }
            _ => panic!("Expected GitHub source, got {:?}", source),
        }
    }

    #[test]
    fn normalize_github_tree_url_resolves_with_subpath() {
        let source = SkillSource::parse(
            "https://github.com/anthropics/skills/tree/main/skills/frontend-design",
        )
        .unwrap();
        match source {
            SkillSource::GitHub {
                owner,
                repo,
                subpath,
            } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath.as_deref(), Some("skills/frontend-design"));
            }
            _ => panic!("Expected GitHub source, got {:?}", source),
        }
    }
}
