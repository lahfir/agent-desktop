use crate::error::AppError;
use serde_json::{json, Value};

const SKILL_DESKTOP_MAIN: &str = include_str!("../../../../skills/agent-desktop/SKILL.md");
const SKILL_DESKTOP_REF_OBSERVATION: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-observation.md");
const SKILL_DESKTOP_REF_INTERACTION: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-interaction.md");
const SKILL_DESKTOP_REF_SYSTEM: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-system.md");
const SKILL_DESKTOP_REF_WORKFLOWS: &str =
    include_str!("../../../../skills/agent-desktop/references/workflows.md");
const SKILL_DESKTOP_REF_MACOS: &str =
    include_str!("../../../../skills/agent-desktop/references/macos.md");

const SKILL_FFI_MAIN: &str = include_str!("../../../../skills/agent-desktop-ffi/SKILL.md");
const SKILL_FFI_REF_BUILD: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/build-and-link.md");
const SKILL_FFI_REF_ERRORS: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/error-handling.md");
const SKILL_FFI_REF_OWNERSHIP: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/ownership.md");
const SKILL_FFI_REF_THREADING: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/threading.md");

struct SkillRef {
    rel_path: &'static str,
    body: &'static str,
}

struct Skill {
    canonical: &'static str,
    aliases: &'static [&'static str],
    summary: &'static str,
    main: &'static str,
    refs: &'static [SkillRef],
}

const SKILLS: &[Skill] = &[
    Skill {
        canonical: "agent-desktop",
        aliases: &["desktop", "agent-desktop"],
        summary: "Primary guide. Snapshot/ref loop, JSON envelope, 53 commands across observation, interaction, keyboard/mouse, app lifecycle, notifications, clipboard, wait.",
        main: SKILL_DESKTOP_MAIN,
        refs: &[
            SkillRef { rel_path: "references/commands-observation.md", body: SKILL_DESKTOP_REF_OBSERVATION },
            SkillRef { rel_path: "references/commands-interaction.md", body: SKILL_DESKTOP_REF_INTERACTION },
            SkillRef { rel_path: "references/commands-system.md", body: SKILL_DESKTOP_REF_SYSTEM },
            SkillRef { rel_path: "references/workflows.md", body: SKILL_DESKTOP_REF_WORKFLOWS },
            SkillRef { rel_path: "references/macos.md", body: SKILL_DESKTOP_REF_MACOS },
        ],
    },
    Skill {
        canonical: "agent-desktop-ffi",
        aliases: &["ffi", "agent-desktop-ffi"],
        summary: "Embedding agent-desktop in another process via the C ABI. Build/link, error propagation, handle ownership, threading rules.",
        main: SKILL_FFI_MAIN,
        refs: &[
            SkillRef { rel_path: "references/build-and-link.md", body: SKILL_FFI_REF_BUILD },
            SkillRef { rel_path: "references/error-handling.md", body: SKILL_FFI_REF_ERRORS },
            SkillRef { rel_path: "references/ownership.md", body: SKILL_FFI_REF_OWNERSHIP },
            SkillRef { rel_path: "references/threading.md", body: SKILL_FFI_REF_THREADING },
        ],
    },
];

pub struct GetArgs {
    pub name: String,
    pub full: bool,
    pub reference: Option<String>,
}

pub fn list() -> Result<Value, AppError> {
    let entries: Vec<Value> = SKILLS
        .iter()
        .map(|s| {
            json!({
                "name": s.canonical,
                "aliases": s.aliases,
                "summary": s.summary,
                "references": s.refs.iter().map(|r| r.rel_path).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({ "skills": entries }))
}

pub fn get(args: GetArgs) -> Result<Value, AppError> {
    let skill = find_skill(&args.name)?;

    if let Some(rel) = args.reference {
        let r = skill
            .refs
            .iter()
            .find(|r| matches_ref(r.rel_path, &rel))
            .ok_or_else(|| {
                let available: Vec<&str> = skill.refs.iter().map(|r| r.rel_path).collect();
                AppError::invalid_input(format!(
                    "Unknown reference '{rel}' for skill '{}'. Available: {}",
                    skill.canonical,
                    available.join(", ")
                ))
            })?;
        return Ok(json!({
            "skill": skill.canonical,
            "reference": r.rel_path,
            "content": r.body,
        }));
    }

    let content = if args.full {
        render_full(skill)
    } else {
        skill.main.to_string()
    };

    Ok(json!({
        "skill": skill.canonical,
        "full": args.full,
        "content": content,
    }))
}

pub fn path() -> Result<Value, AppError> {
    Ok(json!({
        "location": "embedded",
        "note": "Skills are compiled into this binary and are always version-matched. Run `agent-desktop skills get <name>` to print a skill, or redirect into a file to extract a copy.",
        "available": SKILLS.iter().map(|s| s.canonical).collect::<Vec<_>>(),
    }))
}

fn find_skill(name: &str) -> Result<&'static Skill, AppError> {
    let needle = name.trim();
    SKILLS
        .iter()
        .find(|s| s.aliases.iter().any(|a| a.eq_ignore_ascii_case(needle)))
        .ok_or_else(|| {
            let known: Vec<&str> = SKILLS
                .iter()
                .flat_map(|s| s.aliases.iter().copied())
                .collect();
            AppError::invalid_input(format!(
                "Unknown skill '{name}'. Known: {}",
                known.join(", ")
            ))
        })
}

fn matches_ref(rel_path: &str, query: &str) -> bool {
    if rel_path.eq_ignore_ascii_case(query) {
        return true;
    }
    let stem = rel_path
        .rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix(".md").or(Some(f)))
        .unwrap_or(rel_path);
    stem.eq_ignore_ascii_case(query)
}

fn render_full(skill: &Skill) -> String {
    let mut out = String::with_capacity(
        skill.main.len() + skill.refs.iter().map(|r| r.body.len() + 64).sum::<usize>(),
    );
    out.push_str(skill.main);
    for r in skill.refs {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n--- ");
        out.push_str(r.rel_path);
        out.push_str(" ---\n\n");
        out.push_str(r.body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_known_skills() {
        let v = list().expect("list");
        let arr = v["skills"].as_array().expect("array");
        assert!(arr.iter().any(|s| s["name"] == "agent-desktop"));
        assert!(arr.iter().any(|s| s["name"] == "agent-desktop-ffi"));
    }

    #[test]
    fn get_resolves_alias() {
        let v = get(GetArgs {
            name: "desktop".into(),
            full: false,
            reference: None,
        })
        .expect("get");
        assert_eq!(v["skill"], "agent-desktop");
        assert!(v["content"].as_str().unwrap().contains("agent-desktop"));
    }

    #[test]
    fn get_full_inlines_references() {
        let v = get(GetArgs {
            name: "desktop".into(),
            full: true,
            reference: None,
        })
        .expect("get full");
        let content = v["content"].as_str().expect("string");
        assert!(content.contains("--- references/workflows.md ---"));
        assert!(content.contains("--- references/macos.md ---"));
    }

    #[test]
    fn get_specific_reference() {
        let v = get(GetArgs {
            name: "desktop".into(),
            full: false,
            reference: Some("workflows".into()),
        })
        .expect("get ref");
        assert_eq!(v["reference"], "references/workflows.md");
    }

    #[test]
    fn unknown_skill_errors() {
        let err = get(GetArgs {
            name: "nope".into(),
            full: false,
            reference: None,
        })
        .expect_err("should error");
        assert!(format!("{err}").contains("Unknown skill"));
    }

    #[test]
    fn path_lists_canonical_names() {
        let v = path().expect("path");
        assert_eq!(v["location"], "embedded");
        let avail = v["available"].as_array().expect("arr");
        assert!(avail.iter().any(|s| s == "agent-desktop"));
    }
}
