use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

use masonry::app::RenderRoot;

use super::App;
use crate::chat::{self, CursorModel, CursorModels};
use crate::ui::settings::schema::Section;

pub(super) struct CursorModelsState {
    pub(super) key: String,
    pub(super) status: CursorModels,
    tx: Sender<(String, Result<Vec<CursorModel>, String>)>,
    rx: Receiver<(String, Result<Vec<CursorModel>, String>)>,
}

impl Default for CursorModelsState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            key: String::new(),
            status: CursorModels::Locked,
            tx,
            rx,
        }
    }
}

impl App {
    pub(super) fn request_cursor_models(&mut self) {
        let key = self
            .workspace
            .core
            .settings
            .get_config()
            .agent
            .api_key
            .trim()
            .to_owned();
        if key.is_empty() {
            self.cursor_models.key.clear();
            self.cursor_models.status = CursorModels::Locked;
            return;
        }
        if !looks_like_cursor_key(&key) {
            self.cursor_models.key.clear();
            self.cursor_models.status = CursorModels::Locked;
            return;
        }
        if self.cursor_models.key == key
            && matches!(
                self.cursor_models.status,
                CursorModels::Loading | CursorModels::Ready(_)
            )
        {
            return;
        }
        self.cursor_models.key = key.clone();
        self.cursor_models.status = CursorModels::Loading;
        let tx = self.cursor_models.tx.clone();
        let wake = Arc::clone(&self.workspace.wake);
        std::thread::spawn(move || {
            let result = chat::list_models(&key);
            if let Err(error) = &result {
                report!("модели Cursor не загружены: {error}");
            }
            let _ = tx.send((key, result));
            wake();
        });
    }

    pub(super) fn poll_cursor_models(&mut self, root: &mut RenderRoot) {
        let mut latest = None;
        while let Ok(message) = self.cursor_models.rx.try_recv() {
            latest = Some(message);
        }
        let Some((key, result)) = latest else { return };
        if key != self.cursor_models.key {
            return;
        }
        self.cursor_models.status = match result {
            Ok(models) => CursorModels::Ready(models),
            Err(error) => CursorModels::Failed(error),
        };
        self.refresh_section(root, Section::Agent);
    }
}

fn looks_like_cursor_key(key: &str) -> bool {
    let key = key.trim();
    key.len() >= 20
        && (key.starts_with("grok_") || key.starts_with("crsr_") || key.starts_with("cursor_") || key.starts_with("key_"))
}
