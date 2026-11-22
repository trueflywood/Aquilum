use std::path::{Path, PathBuf};

use aquilum_core::Core;
use aquilum_core::files::document::read_text;
use aquilum_core::files::gate;
use aquilum_core::history::Source;
use aquilum_core::search::query::Request;

use super::{Message, Role, Skill};
use crate::i18n::t_with;

pub struct Turn {
    pub messages: Vec<Message>,
    pub changed_paths: Vec<PathBuf>,
}

pub fn run_turn(core: &Core, workspace: &Path, user_text: &str, skills: &[Skill]) -> Turn {
    let text = user_text.trim();
    let mut out = Vec::new();
    let mut changed = Vec::new();
    if text.is_empty() {
        return Turn {
            messages: out,
            changed_paths: changed,
        };
    }

    let active: Vec<&Skill> = skills.iter().filter(|s| s.enabled).collect();
    let skill = pick_skill(&active, text);

    if let Some(skill) = skill {
        out.push(Message {
            role: Role::System,
            content: format!("Skill: {} — {}", skill.name, skill.description),
        });
    }

    if looks_like_search(text) {
        let query = strip_command(text, &["найди", "search", "поиск", "что есть про"]);
        out.push(tool_line("search_notes"));
        let hits = search(core, workspace, &query);
        out.push(Message {
            role: Role::Assistant,
            content: if hits.is_empty() {
                format!("По запросу «{query}» ничего не нашёл.")
            } else {
                let lines = hits
                    .iter()
                    .take(8)
                    .map(|(path, title, snippet)| format!("• **{title}** (`{path}`) — {snippet}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Ищу по базе: «{query}».\n\n{lines}")
            },
        });
        return Turn {
            messages: out,
            changed_paths: changed,
        };
    }

    if looks_like_inbox(text) || skill.is_some_and(|s| s.name.to_lowercase().contains("inbox")) {
        let body = strip_command(text, &["запиши", "зафиксируй", "в inbox", "capture"]);
        out.push(tool_line("append_note"));
        match append_inbox(core, workspace, &body) {
            Ok(path) => {
                changed.push(path.clone());
                out.push(Message {
                    role: Role::Assistant,
                    content: format!("Добавил в Inbox:\n- {body}\n\n`{}`", rel(workspace, &path)),
                });
            }
            Err(error) => out.push(Message {
                role: Role::Assistant,
                content: format!("Не удалось записать в Inbox: {error}"),
            }),
        }
        return Turn {
            messages: out,
            changed_paths: changed,
        };
    }

    if let Some(title) = create_title(text) {
        out.push(tool_line("create_note"));
        match create_note(core, workspace, &title) {
            Ok(path) => {
                changed.push(path.clone());
                out.push(Message {
                    role: Role::Assistant,
                    content: format!("Создал заметку «{title}» (`{}`).", rel(workspace, &path)),
                });
            }
            Err(error) => out.push(Message {
                role: Role::Assistant,
                content: format!("Не удалось создать заметку: {error}"),
            }),
        }
        return Turn {
            messages: out,
            changed_paths: changed,
        };
    }

    let query = text
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.chars().count() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let query = if query.is_empty() {
        text.to_owned()
    } else {
        query
    };
    out.push(tool_line("search_notes"));
    let hits = search(core, workspace, &query);
    if hits.is_empty() {
        out.push(Message {
            role: Role::Assistant,
            content: format!(
                "Локальный агент без внешнего LLM. Примеры: «найди MCP», «запиши идею…», «создай Weekly review».\n\nПо «{query}» совпадений нет."
            ),
        });
    } else {
        let lines = hits
            .iter()
            .take(5)
            .map(|(path, title, snippet)| format!("• **{title}** (`{path}`) — {snippet}"))
            .collect::<Vec<_>>()
            .join("\n");
        out.push(Message {
            role: Role::Assistant,
            content: format!("Нашёл в базе по «{query}»:\n\n{lines}"),
        });
    }
    Turn {
        messages: out,
        changed_paths: changed,
    }
}

fn tool_line(name: &str) -> Message {
    Message {
        role: Role::Tool,
        content: t_with("chat.toolLine", &[("name", name)]),
    }
}

fn pick_skill<'a>(skills: &[&'a Skill], text: &str) -> Option<&'a Skill> {
    if skills.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let mut best: Option<(&'a Skill, i32)> = None;
    for &skill in skills {
        let mut score = 0;
        let hay = format!("{} {} {}", skill.name, skill.description, skill.instructions).to_lowercase();
        for token in lower.split(|c: char| !c.is_alphanumeric()).filter(|t| t.chars().count() > 2) {
            if hay.contains(token) {
                score += 1;
            }
        }
        if contains_any(&lower, &["inbox", "входящ", "зафиксир", "запиши", "capture"])
            && skill.name.to_lowercase().contains("inbox")
        {
            score += 5;
        }
        if contains_any(&lower, &["исслед", "research", "суммар", "синтез"])
            && skill.name.to_lowercase().contains("research")
        {
            score += 5;
        }
        if best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((skill, score));
        }
    }
    match best {
        Some((skill, score)) if score > 0 => Some(skill),
        _ => Some(*skills.first()?),
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn looks_like_search(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("найди")
        || lower.starts_with("search")
        || lower.starts_with("поиск")
        || lower.contains("что есть про")
        || lower.contains("найди заметк")
}

fn looks_like_inbox(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("запиши")
        || lower.starts_with("зафиксируй")
        || lower.starts_with("в inbox")
        || lower.starts_with("capture")
}

fn create_title(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for prefix in ["создай ", "create ", "новая заметка "] {
        if lower.starts_with(prefix) {
            let original = text.get(prefix.len()..).unwrap_or("").trim().trim_matches(['«', '»', '"']);
            if !original.is_empty() {
                return Some(original.to_owned());
            }
        }
    }
    None
}

fn strip_command(text: &str, prefixes: &[&str]) -> String {
    let lower = text.to_lowercase();
    for prefix in prefixes {
        if lower.starts_with(prefix) {
            return text[prefix.len()..]
                .trim()
                .trim_start_matches([':', '-', '—'])
                .trim()
                .to_owned();
        }
    }
    text.to_owned()
}

fn search(core: &Core, workspace: &Path, query: &str) -> Vec<(String, String, String)> {
    let request = Request {
        limit: Some(10),
        ..Request::new(query)
    };
    let Ok(response) = core.search.search(&workspace.to_string_lossy(), &request) else {
        return Vec::new();
    };
    response
        .results
        .into_iter()
        .map(|r| {
            let path = rel(workspace, Path::new(&r.path));
            let title = if r.title.trim().is_empty() {
                path.clone()
            } else {
                r.title
            };
            let snippet = r.snippet.replace('\n', " ");
            let snippet = if snippet.chars().count() > 140 {
                format!("{}…", snippet.chars().take(140).collect::<String>())
            } else {
                snippet
            };
            (path, title, snippet)
        })
        .collect()
}

fn append_inbox(core: &Core, workspace: &Path, text: &str) -> Result<PathBuf, String> {
    let path = workspace.join("Inbox.md");
    if path.exists() {
        let mut content = read_text(&path).unwrap_or_default();
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("\n- {text}\n"));
        gate::write(core, &path, &content, None, Source::Agent, None).map_err(|e| format!("{e:?}"))?;
        Ok(path)
    } else {
        let content = format!("# Inbox\n\n- {text}\n");
        gate::create(core, &path, &content, Source::Agent, None).map_err(|e| format!("{e:?}"))?;
        Ok(path)
    }
}

fn create_note(core: &Core, workspace: &Path, title: &str) -> Result<PathBuf, String> {
    let stem = sanitize(title);
    let mut path = workspace.join(format!("{stem}.md"));
    let mut i = 2;
    while path.exists() {
        path = workspace.join(format!("{stem} {i}.md"));
        i += 1;
    }
    let content = format!("# {title}\n\nСоздано из чата.\n");
    gate::create(core, &path, &content, Source::Agent, None).map_err(|e| format!("{e:?}"))?;
    Ok(path)
}

fn sanitize(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() {
        "note".into()
    } else {
        cleaned.chars().take(80).collect()
    }
}

fn rel(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
