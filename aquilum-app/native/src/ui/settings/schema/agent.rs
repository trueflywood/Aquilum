use std::path::{Path, PathBuf};

use super::*;
use crate::chat::{CursorModels, choice_at, set_choices};
use crate::i18n::{t, t_with};

pub(super) const DEFAULT_SKILLS_FOLDER: &str = "Skills";
const API_KEYS_URL: &str = "https://cursor.com/dashboard/api";

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
        info(
            t("settings.agent.apiKeyPage"),
            Some(t("settings.agent.apiKeyPageHint")),
            Control::Buttons(vec![Button::new(
                t("settings.agent.apiKeyPageButton"),
                Action::OpenUrl(API_KEYS_URL.to_owned()),
            )]),
        ),
        model_row(config, &ctx.cursor_models),
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

fn model_row(config: &AppConfig, models: &CursorModels) -> Row {
    let label = t("settings.agent.model");
    match models {
        CursorModels::Locked => info(
            label,
            Some(t("settings.agent.modelNeedKey")),
            Control::Dropdown {
                options: vec![t("settings.agent.modelLocked")],
                selected: 0,
            },
        ),
        CursorModels::Loading => info(
            label,
            Some(t("settings.agent.modelLoading")),
            Control::Dropdown {
                options: vec![t("settings.agent.modelLoading")],
                selected: 0,
            },
        ),
        CursorModels::Failed(error) => {
            let hint = if error.contains("403") {
                t_with("settings.agent.modelForbidden", &[("error", error)])
            } else {
                t_with("settings.agent.modelFailed", &[("error", error)])
            };
            info(
                label,
                Some(hint),
                Control::Buttons(vec![Button::new(
                    t("settings.agent.modelRetry"),
                    Action::RefreshCursorModels,
                )]),
            )
        }
        CursorModels::Ready(items) => {
            let mut ids = vec![String::new()];
            let mut options = vec![t("settings.agent.modelDefault")];
            for model in items {
                ids.push(model.id.clone());
                options.push(model.label.clone());
            }
            let current = config.agent.model.trim();
            if !current.is_empty() && !ids.iter().any(|id| id == current) {
                ids.push(current.to_owned());
                options.push(current.to_owned());
            }
            let selected = ids.iter().position(|id| id == current).unwrap_or(0);
            set_choices(ids);
            row(
                label,
                Some(t("settings.agent.modelHint")),
                Control::Dropdown { options, selected },
                |c, v| c.agent.model = choice_at(index(v)),
            )
        }
    }
}
