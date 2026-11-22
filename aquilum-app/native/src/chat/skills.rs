use std::fs;
use std::path::{Path, PathBuf};

use aquilum_core::Core;
use aquilum_core::files::gate;
use aquilum_core::history::Source;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Skill {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub tools: Vec<String>,
    pub instructions: String,
}

pub fn write_starters(core: &Core, folder: &Path) {
    if let Err(error) = fs::create_dir_all(folder) {
        report!("папка Skills не создана: {error}");
        return;
    }
    let starters = [
        (
            "Research.md",
            "---\nname: Research\ndescription: Ищет по базе и сохраняет синтез\nenabled: true\ntools: [list_notes, search_notes, read_note, create_note, append_note]\n---\n\nСначала ищи по базе, потом кратко отвечай. Полезный синтез сохраняй новой заметкой или дописывай в Inbox.\n",
        ),
        (
            "Inbox capture.md",
            "---\nname: Inbox capture\ndescription: Фиксирует мысль в Inbox\nenabled: true\ntools: [list_notes, read_note, create_note, append_note]\n---\n\nЕсли пользователь диктует мысль или задачу — оформи Markdown и допиши в Inbox.\n",
        ),
    ];
    for (name, content) in starters {
        let path = folder.join(name);
        if path.exists() {
            continue;
        }
        if let Err(error) = gate::create(core, &path, &content, Source::Me, None) {
            report!("{} не создан: {error:?}", path.display());
        }
    }
}

pub fn load_skills(folder: &Path) -> Vec<Skill> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case("md")) {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(skill) = parse_skill(path, &raw) {
            skills.push(skill);
        }
    }
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    skills
}

fn parse_skill(path: PathBuf, raw: &str) -> Option<Skill> {
    let (meta, body) = split_frontmatter(raw)?;
    let name = meta_string(&meta, "name").unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Skill".into())
    });
    let description = meta_string(&meta, "description").unwrap_or_default();
    let enabled = meta_string(&meta, "enabled")
        .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0" | "no" | "off"))
        .unwrap_or(true);
    let tools = meta_list(&meta, "tools");
    Some(Skill {
        path,
        name,
        description,
        enabled,
        tools,
        instructions: body.trim().to_owned(),
    })
}

fn split_frontmatter(raw: &str) -> Option<(String, String)> {
    let text = raw.trim_start_matches('\u{feff}');
    if !text.starts_with("---") {
        return Some((String::new(), text.to_owned()));
    }
    let rest = text.trim_start_matches("---").trim_start_matches(['\r', '\n']);
    let end = rest.find("\n---")?;
    let meta = rest[..end].to_owned();
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_owned();
    Some((meta, body))
}

fn meta_string(meta: &str, key: &str) -> Option<String> {
    for line in meta.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_owned());
        }
    }
    None
}

fn meta_list(meta: &str, key: &str) -> Vec<String> {
    let Some(value) = meta_string(meta, key) else {
        return Vec::new();
    };
    let value = value.trim().trim_start_matches('[').trim_end_matches(']');
    value
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_owned())
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_skill() {
        let raw = "---\nname: Research\ndescription: Finds notes\nenabled: true\ntools: [search_notes, read_note]\n---\n\nLook first.\n";
        let skill = parse_skill(PathBuf::from("Skills/Research.md"), raw).unwrap();
        assert_eq!(skill.name, "Research");
        assert_eq!(skill.description, "Finds notes");
        assert!(skill.enabled);
        assert_eq!(skill.tools, ["search_notes", "read_note"]);
        assert_eq!(skill.instructions, "Look first.");
    }
}
