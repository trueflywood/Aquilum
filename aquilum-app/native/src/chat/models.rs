use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

const ENDPOINT: &str = "https://api.cursor.com/v1/models";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorModel {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorModels {
    Locked,
    Loading,
    Ready(Vec<CursorModel>),
    Failed(String),
}

#[derive(Deserialize)]
struct ModelsResponse {
    items: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    id: String,
    #[serde(default, rename = "displayName")]
    display_name: String,
}

static CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn choices() -> &'static Mutex<Vec<String>> {
    CHOICES.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn set_choices(ids: Vec<String>) {
    if let Ok(mut guard) = choices().lock() {
        *guard = ids;
    }
}

pub fn choice_at(index: usize) -> String {
    choices()
        .lock()
        .ok()
        .and_then(|guard| guard.get(index).cloned())
        .unwrap_or_default()
}

pub fn list(api_key: &str) -> Result<Vec<CursorModel>, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{key}:"));
    let response = aquilum_core::web_agent::web_agent("cursor-models")
        .get(ENDPOINT)
        .set("Authorization", &format!("Basic {auth}"))
        .set("Accept", "application/json")
        .call()
        .map_err(|error| error.to_string())?;
    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {}", response.status()));
    }
    let body: ModelsResponse = serde_json::from_reader(response.into_reader()).map_err(|error| error.to_string())?;
    let mut models = Vec::new();
    for item in body.items {
        let id = item.id.trim().to_owned();
        if id.is_empty() {
            continue;
        }
        let label = if item.display_name.trim().is_empty() {
            id.clone()
        } else {
            item.display_name.trim().to_owned()
        };
        models.push(CursorModel { id, label });
    }
    if models.is_empty() {
        return Err("empty list".into());
    }
    Ok(models)
}
