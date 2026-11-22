mod agent;
mod cloud;
mod models;
mod skills;
mod ui;

use masonry::core::WidgetId;

use crate::ui::editor::TextEditor;
use crate::ui::handle::Handle;
use crate::ui::search::Placeholder;

pub use agent::{prompt_context, run_turn};
pub use cloud::{CloudReply, ask as ask_cloud};
pub use models::{CursorModel, CursorModels, choice_at, list as list_models, set_choices};
pub use skills::{Skill, load_skills, write_starters};
pub use ui::{ChatBody, body};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Default)]
pub struct ChatState {
    pub messages: Vec<Message>,
    pub busy: bool,
    pub draft: String,
    pub cloud_agent_id: Option<String>,
    pub generation: u64,
}

pub struct ChatBindings {
    pub field: Handle<TextEditor>,
    pub placeholder: Handle<Placeholder>,
    pub send: WidgetId,
    pub clear: WidgetId,
}

