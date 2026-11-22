use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use serde_json::Value;

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
    #[serde(default)]
    items: Vec<ModelItem>,
    #[serde(default)]
    models: Vec<Value>,
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
    match fetch(key, true) {
        Ok(models) => Ok(models),
        Err(basic_error) => match fetch(key, false) {
            Ok(models) => Ok(models),
            Err(bearer_error) => Err(format!("{basic_error}; bearer: {bearer_error}")),
        },
    }
}

fn fetch(key: &str, basic: bool) -> Result<Vec<CursorModel>, String> {
    let agent = aquilum_core::web_agent::web_agent("cursor-models");
    let mut request = agent.get(ENDPOINT).set("Accept", "application/json");
    request = if basic {
        let auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{key}:"));
        request.set("Authorization", &format!("Basic {auth}"))
    } else {
        request.set("Authorization", &format!("Bearer {key}"))
    };
    let response = request.call().map_err(|error| error.to_string())?;
    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {}", response.status()));
    }
    let body: ModelsResponse = serde_json::from_reader(response.into_reader()).map_err(|error| error.to_string())?;
    let mut models = Vec::new();
    for item in body.items {
        push_model(&mut models, item.id, item.display_name);
    }
    for value in body.models {
        match value {
            Value::String(id) => push_model(&mut models, id, String::new()),
            Value::Object(map) => {
                let id = map.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                let label = map
                    .get("displayName")
                    .or_else(|| map.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                push_model(&mut models, id, label);
            }
            _ => {}
        }
    }
    if models.is_empty() {
        return Err("empty list".into());
    }
    Ok(models)
}

fn push_model(models: &mut Vec<CursorModel>, id: String, label: String) {
    let id = id.trim().to_owned();
    if id.is_empty() {
        return;
    }
    let label = if label.trim().is_empty() {
        id.clone()
    } else {
        label.trim().to_owned()
    };
    if models.iter().any(|model| model.id == id) {
        return;
    }
    models.push(CursorModel { id, label });
}
