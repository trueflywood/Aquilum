use std::path::{Path, PathBuf};

use super::*;
use crate::i18n::{t, t_with};

pub(super) const DEFAULT_SKILLS_FOLDER: &str = "Skills";

pub fn skills_path(workspace: &Path, folder: &str) -> PathBuf {
    let value = folder.trim();
    let drive = value.len() > 2 && value.as_bytes()[0].is_ascii_alphabetic() && value.as_bytes()[1] == b':';
    let absolute = value.starts_with('/') || value.starts_with("\\\\") || drive && value[2..].starts_with(['/', '\\']);
    if absolute {
        return PathBuf::from(value.trim_end_matches(['/', '\\']));
    }
    let normalized = value.trim_matches(['/', '\\']);
    workspace.join(if normalized.is_empty() { DEFAULT_SKILLS_FOLDER } else { normalized })
}

pub(super) fn agent(config: &AppConfig, ctx: &Context) -> Vec<Block> {
    let mut rows = vec![
        row(
            t("settings.agent.enabled"),
            Some(t("settings.agent.enabledHint")),
            Control::Switch(config.agent.enabled),
            |c, v| c.agent.enabled = flag(v),
        ),
        row(
            t("settings.agent.apiKey"),
            Some(t("settings.agent.apiKeyHint")),
            Control::Text {
                value: config.agent.api_key.clone(),
                commit: false,
            },
            |c, v| c.agent.api_key = text(v),
        ),
        row(
            t("settings.agent.model"),
            Some(t("settings.agent.modelHint")),
            Control::Text {
                value: config.agent.model.clone(),
                commit: false,
            },
            |c, v| c.agent.model = text(v),
        ),
        row(
            t("settings.agent.skillsFolder"),
            Some(t("settings.agent.skillsFolderHint")),
            Control::Text {
                value: config.agent.skills.folder.clone(),
                commit: false,
            },
            |c, v| c.agent.skills.folder = text(v),
        ),
    ];
    if let Some(workspace) = &ctx.workspace
        && !skills_path(workspace, &config.agent.skills.folder).is_dir()
    {
        let folder = config.agent.skills.folder.trim();
        let folder = if folder.is_empty() {
            DEFAULT_SKILLS_FOLDER
        } else {
            folder
        };
        rows.push(info(
            t("settings.agent.skillsCreate"),
            Some(t_with("settings.agent.skillsCreateHint", &[("folder", folder)])),
            Control::Buttons(vec![Button::new(
                t("settings.agent.skillsCreateButton"),
                Action::CreateSkills,
            )]),
        ));
    }
    vec![Block {
        title: t("settings.agent.section"),
        rows,
    }]
}
