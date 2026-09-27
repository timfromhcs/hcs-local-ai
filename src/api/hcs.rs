use axum::extract::{Path as AxPath, Query, State};
use axum::http::StatusCode;
use axum::response::{sse::Event, Sse};
use axum::Json;
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::agent::{AgentStep, ToolCall};
use crate::api::AppState;
use crate::doctor::Doctor;
use crate::models::ModelLifecycleState;
use crate::openjev::OpenJevDecisionRequest;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct MemoryQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryInsertRequest {
    pub category: String,
    pub key: String,
    pub content: String,
    pub provenance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct AgentRunRequest {
    pub prompt: String,
    pub model: Option<String>,
}

// GET /hcs/v1/system
pub async fn system_info(State(state): State<AppState>) -> Json<Value> {
    let stats = state.resource.get_stats();
    let models = state.registry.list_models();
    let loaded: Vec<String> = models.iter()
        .filter(|m| m.state == ModelLifecycleState::Ready || m.state == ModelLifecycleState::Active)
        .map(|m| m.manifest.id.clone())
        .collect();

    Json(serde_json::json!({
        "status": "healthy",
        "version": "1.0.0",
        "hardware": {
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "vulkan": true,
            "unified_memory": true,
            "total_ram_mb": stats.total_memory_mb,
            "used_ram_mb": stats.used_memory_mb,
            "available_ram_mb": stats.available_memory_mb,
            "ram_percent": stats.memory_usage_percent,
            "cpu_cores": stats.cpu_count,
            "cpu_percent": stats.cpu_usage_percent,
        },
        "loaded_models": loaded,
        "active_heavy_count": stats.active_heavy_count,
        "max_heavy_active": stats.max_heavy_active,
    }))
}

// GET /hcs/v1/doctor
pub async fn doctor_report(State(state): State<AppState>) -> Json<Value> {
    let report = Doctor::run_diagnostics(&state.config, &state.registry);
    Json(serde_json::to_value(report).unwrap())
}

// GET /hcs/v1/models
pub async fn list_hcs_models(State(state): State<AppState>) -> Json<Value> {
    let models = state.registry.list_models();
    Json(serde_json::json!({ "models": models }))
}

// POST /hcs/v1/models/:id/load
pub async fn load_model(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let resolved = state.registry.resolve_alias(&id);
    match state.ensure_worker(&resolved).await {
        Ok(port) => Ok(Json(serde_json::json!({
            "status": "ready",
            "model_id": resolved,
            "port": port
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )),
    }
}

// POST /hcs/v1/models/:id/unload
pub async fn unload_model(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Json<Value> {
    let resolved = state.registry.resolve_alias(&id);
    let mut workers = state.active_workers.lock().await;
    if let Some(mut w) = workers.remove(&resolved) {
        w.stop().await;
        if crate::resource::ResourceManager::is_heavy_model(&resolved) {
            state.resource.release_heavy_permit();
        }
    }
    state.registry.update_state(&resolved, ModelLifecycleState::Cold);
    Json(serde_json::json!({ "status": "unloaded", "model_id": resolved }))
}

// POST /hcs/v1/decision (OpenJev contract endpoint)
pub async fn decision_contract(
    State(state): State<AppState>,
    Json(req): Json<OpenJevDecisionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    req.validate().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    let target_model = "hcs-judge";
    let port = state.ensure_worker(target_model).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    let prompt = req.render_prompt();

    // Call llama-server with temperature 0 as mandated by OpenJev contract
    let client = reqwest::Client::new();
    let worker_url = format!("http://127.0.0.1:{}/completion", port);

    let forward_body = serde_json::json!({
        "prompt": prompt,
        "temperature": 0.0,
        "n_predict": 16,
        "stop": ["\n", "."],
    });

    let resp = client.post(&worker_url)
        .json(&forward_body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": { "message": e.to_string() } })),
            )
        })?;

    let result_json: Value = resp.json().await.unwrap_or_default();
    let raw_text = result_json.get("content").and_then(|s| s.as_str()).unwrap_or("");

    let decision_resp = req.parse_output(raw_text);
    state.broadcast_event("decision", &decision_resp);

    Ok(Json(serde_json::to_value(decision_resp).unwrap()))
}

// Memory APIs
pub async fn get_memory(
    State(state): State<AppState>,
    Query(q): Query<MemoryQuery>,
) -> Json<Value> {
    let res = if let Some(query_str) = q.q {
        state.db.search_memory(&query_str, q.category.as_deref()).unwrap_or_default()
    } else {
        state.db.list_memory(q.limit.unwrap_or(50)).unwrap_or_default()
    };
    Json(serde_json::json!({ "entries": res }))
}

pub async fn store_memory(
    State(state): State<AppState>,
    Json(payload): Json<MemoryInsertRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prov = payload.provenance.unwrap_or_else(|| "api".to_string());
    match state.db.upsert_memory(&payload.category, &payload.key, &payload.content, &prov) {
        Ok(entry) => {
            state.broadcast_event("memory", &entry);
            Ok(Json(serde_json::to_value(entry).unwrap()))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )),
    }
}

pub async fn delete_memory(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Json<Value> {
    let ok = state.db.delete_memory(&id).unwrap_or(false);
    Json(serde_json::json!({ "deleted": ok, "id": id }))
}

// Agent APIs
pub async fn run_agent(
    State(state): State<AppState>,
    Json(payload): Json<AgentRunRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut task = state.agent.create_task(&payload.prompt);
    task.status = "running".to_string();
    state.broadcast_event("agent", &task);

    let tools = state.agent.executor.list_tools();
    let openai_tools: Vec<Value> = tools.into_iter().map(|t| {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters
            }
        })
    }).collect();

    let model_req = payload.model.as_deref().unwrap_or("hcs-coder");
    let target_model = state.registry.resolve_alias(model_req);
    let port = state.ensure_worker(&target_model).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_default();
    let worker_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let mut messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are HCS Autonomous Software Engineering Agent. Plan carefully, use available tools, and solve coding and test tasks."
        }),
        serde_json::json!({
            "role": "user",
            "content": payload.prompt
        }),
    ];

    let max_steps = 5;
    let mut final_content = String::new();

    for step_idx in 0..max_steps {
        let chat_req = serde_json::json!({
            "model": target_model,
            "messages": messages,
            "tools": openai_tools,
            "temperature": 0.1
        });

        let resp = client.post(&worker_url).json(&chat_req).send().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": { "message": e.to_string() } })),
            )
        })?;

        let worker_json: Value = resp.json().await.unwrap_or_default();
        let choice = worker_json.get("choices").and_then(|c| c.get(0));
        let msg = choice.and_then(|c0| c0.get("message"));

        if let Some(m) = msg {
            let content = m.get("content").and_then(|s| s.as_str()).unwrap_or("");
            if !content.is_empty() {
                final_content = content.to_string();
            }

            // Check if tools were called
            if let Some(tool_calls_val) = m.get("tool_calls").and_then(|v| v.as_array()) {
                if !tool_calls_val.is_empty() {
                    // Push assistant message with tool calls
                    messages.push(m.clone());

                    for tc in tool_calls_val {
                        let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("call_1").to_string();
                        let fn_obj = tc.get("function");
                        let fn_name = fn_obj.and_then(|f| f.get("name")).and_then(|s| s.as_str()).unwrap_or("");
                        let args_str = fn_obj.and_then(|f| f.get("arguments")).and_then(|s| s.as_str()).unwrap_or("{}");
                        let args: Value = serde_json::from_str(args_str).unwrap_or_default();

                        let tool_call = ToolCall {
                            id: id.clone(),
                            name: fn_name.to_string(),
                            arguments: args,
                        };

                        let result = state.agent.executor.execute(&tool_call).await;

                        task.steps_taken.push(AgentStep {
                            step_index: step_idx + 1,
                            thought: content.to_string(),
                            tool_call: Some(tool_call),
                            tool_result: Some(result.clone()),
                        });

                        // Self-healing diagnosis if error occurred
                        if !result.success {
                            if let Some(ref err) = result.error {
                                let diag = state.agent.self_healing.diagnose_failure(err);
                                info!("Self-healing diagnosis: {}", diag);
                            }
                        }

                        // Feed tool result back to the model
                        messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": id,
                            "content": if result.success { result.output } else { result.error.unwrap_or_default() }
                        }));
                    }

                    state.broadcast_event("agent", &task);
                    continue; // Loop to next step with tool results
                }
            }
        }

        // If no more tool calls or text response finished
        break;
    }

    task.status = "completed".to_string();
    task.result = Some(final_content);
    state.broadcast_event("agent", &task);

    Ok(Json(serde_json::to_value(task).unwrap()))
}

// Telemetry & Requests
pub async fn get_telemetry(State(state): State<AppState>) -> Json<Value> {
    let snap = state.telemetry.snapshot();
    Json(serde_json::to_value(snap).unwrap())
}

pub async fn get_recent_requests(
    State(state): State<AppState>,
    Query(q): Query<MemoryQuery>,
) -> Json<Value> {
    let reqs = state.db.get_recent_requests(q.limit.unwrap_or(50)).unwrap_or_default();
    Json(serde_json::json!({ "requests": reqs }))
}

// Key Management
pub async fn list_keys(State(state): State<AppState>) -> Json<Value> {
    let keys = state.db.list_api_keys().unwrap_or_default();
    Json(serde_json::json!({ "keys": keys }))
}

pub async fn create_key(
    State(state): State<AppState>,
    Json(payload): Json<CreateKeyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let raw_key = format!("hcs-key-{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let perms = payload.permissions.unwrap_or_else(|| vec!["*".to_string()]);
    match state.db.insert_api_key(&payload.name, &raw_key, &perms) {
        Ok(rec) => Ok(Json(serde_json::json!({
            "key": rec,
            "raw_key": raw_key,
            "raw_secret_key": raw_key,
            "warning": "Store this key safely. It will not be shown again."
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )),
    }
}

pub async fn revoke_key(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Json<Value> {
    let ok = state.db.revoke_api_key(&id).unwrap_or(false);
    Json(serde_json::json!({ "revoked": ok, "id": id }))
}

// SSE live events stream
pub async fn live_events(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| {
        match msg {
            Ok(data) => Some(Ok(Event::default().data(data))),
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)))
}
