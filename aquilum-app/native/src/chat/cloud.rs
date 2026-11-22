use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

const AGENTS: &str = "https://api.cursor.com/v1/agents";
const POLL_EVERY: Duration = Duration::from_secs(2);
const POLL_BUDGET: Duration = Duration::from_secs(180);

#[derive(Clone, Debug)]
pub struct CloudReply {
    pub agent_id: String,
    pub text: String,
}

#[derive(Deserialize)]
struct CreateResponse {
    agent: AgentRef,
    run: RunRef,
}

#[derive(Deserialize)]
struct FollowUpResponse {
    run: RunRef,
}

#[derive(Deserialize)]
struct AgentRef {
    id: String,
}

#[derive(Deserialize)]
struct RunRef {
    id: String,
}

#[derive(Deserialize)]
struct RunStatus {
    #[serde(default)]
    status: String,
    #[serde(default)]
    result: Option<String>,
}

pub fn ask(
    api_key: &str,
    model: &str,
    agent_id: Option<&str>,
    prompt: &str,
) -> Result<CloudReply, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("empty prompt".into());
    }

    match agent_id {
        Some(id) if !id.is_empty() => match follow_up(key, id, prompt) {
            Ok(reply) => Ok(reply),
            Err(error) if recoverable_agent_error(&error) => create(key, model, prompt),
            Err(error) => Err(error),
        },
        _ => create(key, model, prompt),
    }
}

fn create(api_key: &str, model: &str, prompt: &str) -> Result<CloudReply, String> {
    let mut body = json!({
        "prompt": { "text": prompt },
        "name": "Aquilum chat",
        "mode": "plan",
    });
    if let Some(model_id) = normalized_model(model) {
        body["model"] = json!({ "id": model_id });
    }
    let (agent_id, run_id) = match post_json(api_key, AGENTS, &body) {
        Ok(value) => parse_create(&value)?,
        Err(basic_error) => match post_json_bearer(api_key, AGENTS, &body) {
            Ok(value) => parse_create(&value)?,
            Err(bearer_error) => return Err(format!("{basic_error}; bearer: {bearer_error}")),
        },
    };
    let text = poll_result(api_key, &agent_id, &run_id)?;
    Ok(CloudReply { agent_id, text })
}

fn follow_up(api_key: &str, agent_id: &str, prompt: &str) -> Result<CloudReply, String> {
    let url = format!("{AGENTS}/{agent_id}/runs");
    let body = json!({
        "prompt": { "text": prompt },
    });
    let run_id = match post_json(api_key, &url, &body) {
        Ok(value) => parse_follow_up(&value)?,
        Err(basic_error) => match post_json_bearer(api_key, &url, &body) {
            Ok(value) => parse_follow_up(&value)?,
            Err(bearer_error) => return Err(format!("{basic_error}; bearer: {bearer_error}")),
        },
    };
    let text = poll_result(api_key, agent_id, &run_id)?;
    Ok(CloudReply {
        agent_id: agent_id.to_owned(),
        text,
    })
}

fn poll_result(api_key: &str, agent_id: &str, run_id: &str) -> Result<String, String> {
    let url = format!("{AGENTS}/{agent_id}/runs/{run_id}");
    let deadline = Instant::now() + POLL_BUDGET;
    loop {
        let run = match get_json(api_key, &url) {
            Ok(value) => parse_run(&value)?,
            Err(basic_error) => match get_json_bearer(api_key, &url) {
                Ok(value) => parse_run(&value)?,
                Err(bearer_error) => return Err(format!("{basic_error}; bearer: {bearer_error}")),
            },
        };
        if is_terminal(&run.status) {
            if run.status == "FINISHED" {
                let text = run.result.unwrap_or_default().trim().to_owned();
                if text.is_empty() {
                    return Err("run finished without result".into());
                }
                return Ok(text);
            }
            let detail = run.result.unwrap_or_default();
            if detail.trim().is_empty() {
                return Err(format!("run ended with status {}", run.status));
            }
            return Err(format!("run {}: {}", run.status, detail.trim()));
        }
        if Instant::now() >= deadline {
            return Err(format!("run timed out while {}", run.status));
        }
        thread::sleep(POLL_EVERY);
    }
}

fn parse_create(value: &Value) -> Result<(String, String), String> {
    if let Ok(parsed) = serde_json::from_value::<CreateResponse>(value.clone()) {
        return Ok((parsed.agent.id, parsed.run.id));
    }
    let agent_id = value
        .pointer("/agent/id")
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let run_id = value
        .pointer("/run/id")
        .or_else(|| value.pointer("/agent/latestRunId"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if agent_id.is_empty() || run_id.is_empty() {
        return Err(format!("unexpected create response: {value}"));
    }
    Ok((agent_id, run_id))
}

fn parse_follow_up(value: &Value) -> Result<String, String> {
    if let Ok(parsed) = serde_json::from_value::<FollowUpResponse>(value.clone()) {
        return Ok(parsed.run.id);
    }
    value
        .pointer("/run/id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| format!("unexpected follow-up response: {value}"))
}

fn parse_run(value: &Value) -> Result<RunStatus, String> {
    serde_json::from_value(value.clone()).map_err(|error| format!("bad run payload: {error}; {value}"))
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "FINISHED" | "ERROR" | "CANCELLED" | "EXPIRED")
}

fn normalized_model(model: &str) -> Option<&str> {
    let model = model.trim();
    if model.is_empty() {
        None
    } else {
        Some(model)
    }
}

fn recoverable_agent_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("404")
        || lower.contains("not_found")
        || lower.contains("agent_not_found")
        || lower.contains("stopped")
        || lower.contains("archived")
}

fn post_json(api_key: &str, url: &str, body: &Value) -> Result<Value, String> {
    call(api_key, url, Some(body), true)
}

fn post_json_bearer(api_key: &str, url: &str, body: &Value) -> Result<Value, String> {
    call(api_key, url, Some(body), false)
}

fn get_json(api_key: &str, url: &str) -> Result<Value, String> {
    call(api_key, url, None, true)
}

fn get_json_bearer(api_key: &str, url: &str) -> Result<Value, String> {
    call(api_key, url, None, false)
}

fn call(api_key: &str, url: &str, body: Option<&Value>, basic: bool) -> Result<Value, String> {
    let agent = aquilum_core::web_agent::web_agent("cursor-cloud");
    let mut request = if body.is_some() {
        agent.post(url)
    } else {
        agent.get(url)
    };
    request = request.set("Accept", "application/json");
    if body.is_some() {
        request = request.set("Content-Type", "application/json");
    }
    request = if basic {
        let auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{api_key}:"));
        request.set("Authorization", &format!("Basic {auth}"))
    } else {
        request.set("Authorization", &format!("Bearer {api_key}"))
    };
    let response = match body {
        Some(payload) => request.send_string(&payload.to_string()),
        None => request.call(),
    };
    let response = match response {
        Ok(response) => response,
        Err(error) => return Err(format_ureq(error)),
    };
    if !(200..300).contains(&response.status()) {
        let status = response.status();
        let body = response.into_string().unwrap_or_default();
        return Err(format_http(status, &body));
    }
    serde_json::from_reader(response.into_reader()).map_err(|error| error.to_string())
}

fn format_ureq(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(status, response) => {
            let body = response.into_string().unwrap_or_default();
            format_http(status, &body)
        }
        other => other.to_string(),
    }
}

fn format_http(status: u16, body: &str) -> String {
    let body = body.trim();
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        let code = value.get("code").and_then(Value::as_str).unwrap_or("");
        let message = value.get("message").and_then(Value::as_str).unwrap_or(body);
        if code.is_empty() {
            format!("HTTP {status}: {message}")
        } else {
            format!("HTTP {status} ({code}): {message}")
        }
    } else if body.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {body}")
    }
}
