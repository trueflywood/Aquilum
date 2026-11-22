use masonry::app::RenderRoot;
use masonry::core::{ErasedAction, WidgetId};

use super::{App, Outcome};
use crate::chat::{self, ChatBody, ChatState, Message, Role};
use crate::ui::editor::{TextAction, TextEditor};
use crate::ui::search::Placeholder;
use crate::ui::settings::schema;
use crate::ui::widgets::{Pressed, Slot};

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
        let turn = chat::run_turn(&self.workspace.core, &workspace, &text, &skills);
        self.chat.messages.extend(turn.messages);
        self.chat.busy = false;
        self.render_chat(root);
        if !turn.changed_paths.is_empty() {
            self.tree.reload(root);
        }
    }

    fn clear_chat(&mut self, root: &mut RenderRoot) {
        self.chat = ChatState::default();
        self.render_chat(root);
    }
}
