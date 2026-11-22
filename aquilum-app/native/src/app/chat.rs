use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

use masonry::app::RenderRoot;
use masonry::core::{ErasedAction, WidgetId};

use super::{App, Outcome};
use crate::chat::{self, ChatBody, ChatState, Message, Role};
use crate::ui::editor::{TextAction, TextEditor};
use crate::ui::search::Placeholder;
use crate::ui::settings::schema;
use crate::ui::widgets::{Pressed, Slot};

pub(super) struct ChatCloudState {
    tx: Sender<CloudOutcome>,
    rx: Receiver<CloudOutcome>,
}

struct CloudOutcome {
    generation: u64,
    result: Result<chat::CloudReply, String>,
}

impl Default for ChatCloudState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }
}

impl App {
    pub(super) fn render_chat(&mut self, root: &mut RenderRoot) {
        let Some(chrome) = &self.chrome else { return };
        let config = self.workspace.core.settings.get_config();
        let has_base = self.tree.root().is_some();
        let enabled = config.agent.enabled;
        let (body, ChatBody { bindings }) = chat::body(&self.chat, enabled, has_base);
        chrome.panel_parts.body.edit(root, |mut slot| Slot::set_child(&mut slot, body));
        self.chat_body = Some(bindings);
    }

    pub(in crate::app) fn on_chat_action(&mut self, root: &mut RenderRoot, id: WidgetId, action: &ErasedAction) -> Option<Outcome> {
        if self.panel_mode() != crate::links::Mode::Chat {
            return None;
        }
        let bindings = self.chat_body.as_ref()?;
        if id == bindings.field.id() {
            match action.downcast_ref::<TextAction>() {
                Some(TextAction::Changed(text)) => {
                    let empty = text.is_empty();
                    self.chat.draft = text.clone();
                    bindings.placeholder.edit(root, |mut p| Placeholder::set_empty(&mut p, empty));
                }
                Some(TextAction::Entered(text)) => {
                    self.chat.draft = text.clone();
                    self.send_chat(root);
                }
                _ => {}
            }
            return Some(Outcome::default());
        }
        if action.downcast_ref::<Pressed>().is_none() {
            return None;
        }
        if id == bindings.send {
            self.send_chat(root);
            return Some(Outcome::default());
        }
        if id == bindings.clear {
            self.clear_chat(root);
            return Some(Outcome::default());
        }
        None
    }

    pub(super) fn poll_chat_cloud(&mut self, root: &mut RenderRoot) {
        let mut latest = None;
        while let Ok(message) = self.chat_cloud.rx.try_recv() {
            latest = Some(message);
        }
        let Some(outcome) = latest else { return };
        if outcome.generation != self.chat.generation || !self.chat.busy {
            return;
        }
        match outcome.result {
            Ok(reply) => {
                self.chat.cloud_agent_id = Some(reply.agent_id);
                self.chat.messages.push(Message {
                    role: Role::Assistant,
                    content: reply.text,
                });
            }
            Err(error) => {
                report!("Cursor Cloud Agents: {error}");
                self.chat.messages.push(Message {
                    role: Role::System,
                    content: cloud_error_text(&error),
                });
            }
        }
        self.chat.busy = false;
        self.render_chat(root);
    }

    fn send_chat(&mut self, root: &mut RenderRoot) {
        if self.chat.busy {
            return;
        }
        let config = self.workspace.core.settings.get_config();
        if !config.agent.enabled {
            self.chat.messages.push(Message {
                role: Role::System,
                content: crate::i18n::t("chat.disabled"),
            });
            self.render_chat(root);
            return;
        }
        let Some(workspace) = self.tree.root().map(std::path::Path::to_path_buf) else {
            self.chat.messages.push(Message {
                role: Role::System,
                content: crate::i18n::t("chat.noBase"),
            });
            self.render_chat(root);
            return;
        };

        let text = self.chat.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }

        self.chat.messages.push(Message {
            role: Role::User,
            content: text.clone(),
        });
        self.chat.draft.clear();
        self.chat.busy = true;
        if let Some(bindings) = &self.chat_body {
            bindings.field.edit(root, |mut field| TextEditor::reset_text(&mut field, ""));
            bindings.placeholder.edit(root, |mut p| Placeholder::set_empty(&mut p, true));
        }
        self.render_chat(root);

        let folder = schema::skills_path(&workspace, &config.agent.skills.folder);
        let skills = chat::load_skills(&folder);
        let api_key = config.agent.api_key.trim().to_owned();
        if api_key.is_empty() {
            let turn = chat::run_turn(&self.workspace.core, &workspace, &text, &skills);
            self.chat.messages.extend(turn.messages);
            self.chat.busy = false;
            self.render_chat(root);
            if !turn.changed_paths.is_empty() {
                self.tree.reload(root);
            }
            return;
        }

        let prompt = chat::prompt_context(&self.workspace.core, &workspace, &text, &skills);
        let model = config.agent.model.clone();
        let agent_id = self.chat.cloud_agent_id.clone();
        self.chat.generation = self.chat.generation.wrapping_add(1);
        let generation = self.chat.generation;
        let tx = self.chat_cloud.tx.clone();
        let wake = Arc::clone(&self.workspace.wake);
        std::thread::spawn(move || {
            let result = chat::ask_cloud(&api_key, &model, agent_id.as_deref(), &prompt);
            if let Err(error) = &result {
                report!("Cursor Cloud Agents: {error}");
            }
            let _ = tx.send(CloudOutcome { generation, result });
            wake();
        });
    }

    fn clear_chat(&mut self, root: &mut RenderRoot) {
        self.chat = ChatState {
            generation: self.chat.generation.wrapping_add(1),
            ..ChatState::default()
        };
        self.render_chat(root);
    }
}

fn cloud_error_text(error: &str) -> String {
    match error.trim() {
        "USAGE_LIMIT" => crate::i18n::t("chat.cloudUsageLimit"),
        "KEY_NOT_SUPPORTED" => crate::i18n::t("chat.cloudKeyUnsupported"),
        other => crate::i18n::t_with("chat.cloudError", &[("error", other)]),
    }
}
