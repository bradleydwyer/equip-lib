# equip-lib

Shared Rust library for parsing and working with [SKILL.md](https://github.com/bradleydwyer/equip) files.

## What it does

- **Parse SKILL.md frontmatter** — extract `name`, `description`, `version` from YAML frontmatter
- **Resolve skill sources** — parse GitHub shorthand (`owner/repo`), full URLs, git URLs, and local paths into structured types
- **Discover skills** — scan directories for SKILL.md files

## Usage

```toml
[dependencies]
equip-lib = { git = "https://github.com/bradleydwyer/equip-lib" }
```

```rust
use equip_lib::skill::parse_frontmatter;
use equip_lib::source::SkillSource;

// Parse a SKILL.md file
let content = std::fs::read_to_string("SKILL.md").unwrap();
let fm = parse_frontmatter(&content).unwrap();
println!("{}: {}", fm.name, fm.description);

// Parse a skill source
let source = SkillSource::parse("anthropics/skills/pdf").unwrap();
```

## SKILL.md format

```markdown
---
name: my-skill
description: What this skill does
version: 1.0.0
---

Skill instructions go here...
```

The parser handles quoted values, folded (`>`) and literal (`|`) YAML scalars, and various edge cases.

## License

MIT
