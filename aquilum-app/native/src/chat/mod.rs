mod agent;
mod skills;
mod ui;

use masonry::core::WidgetId;

use crate::ui::editor::TextEditor;
use crate::ui::handle::Handle;
use crate::ui::search::Placeholder;

pub use agent::run_turn;
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
}

pub struct ChatBindings {
    pub field: Handle<TextEditor>,
    pub placeholder: Handle<Placeholder>,
    pub send: WidgetId,
    pub clear: WidgetId,
}

