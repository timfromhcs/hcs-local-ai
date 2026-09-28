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
        "version": env!("CARGO_PKG_VERSION"),
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

// ============================================================================
// HCS v2.0.0 Endpoints (J-Space, Jev Delegation Pipeline, Brain Auto-Learning)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateJSpaceRequest {
    pub title: Option<String>,
    pub initial_goal: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetJSpaceStateRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct AppendJSpaceTurnRequest {
    pub role: String,
    pub model: String,
    pub content: String,
    pub tool_calls: Option<Value>,
    pub tool_results: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct JevDelegateRequest {
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct JevRatePlanRequest {
    pub goal: String,
    pub plans: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BrainLearnRequest {
    pub category: String,
    pub pattern: String,
    pub solution: String,
    pub confidence: Option<f32>,
}

// POST /hcs/v2/jspace/sessions
pub async fn create_jspace_session(
    State(state): State<AppState>,
    Json(req): Json<CreateJSpaceRequest>,
) -> Json<Value> {
    let session = state.jspace.create_session(req.title);
    if let Some(goal) = req.initial_goal {
        state.jspace.add_goal(&session.id, &goal);
    }
    let refreshed = state.jspace.get_session(&session.id).unwrap_or(session);
    Json(serde_json::to_value(refreshed).unwrap())
}

// GET /hcs/v2/jspace/sessions
pub async fn list_jspace_sessions(State(state): State<AppState>) -> Json<Value> {
    let sessions = state.jspace.list_sessions();
    Json(serde_json::json!({ "sessions": sessions, "count": sessions.len() }))
}

// GET /hcs/v2/jspace/sessions/:id
pub async fn get_jspace_session(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(session) = state.jspace.get_session(&id) {
        Ok(Json(serde_json::to_value(session).unwrap()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// POST /hcs/v2/jspace/sessions/:id/state
pub async fn set_jspace_state(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
    Json(req): Json<SetJSpaceStateRequest>,
) -> Result<Json<Value>, StatusCode> {
    if state.jspace.set_shared_state(&id, &req.key, &req.value) {
        Ok(Json(serde_json::json!({ "success": true, "session_id": id, "key": req.key })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// POST /hcs/v2/jspace/sessions/:id/turns
pub async fn append_jspace_turn(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
    Json(req): Json<AppendJSpaceTurnRequest>,
) -> Result<Json<Value>, StatusCode> {
    let turn = crate::j_space::JSpaceTurn {
        role: req.role,
        model: req.model,
        content: req.content,
        tool_calls: req.tool_calls,
        tool_results: req.tool_results,
        timestamp: chrono::Utc::now(),
    };
    if state.jspace.append_turn(&id, turn) {
        Ok(Json(serde_json::json!({ "success": true, "session_id": id })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// DELETE /hcs/v2/jspace/sessions/:id
pub async fn delete_jspace_session(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Json<Value> {
    let ok = state.jspace.delete_session(&id);
    Json(serde_json::json!({ "deleted": ok, "id": id }))
}

// POST /hcs/v2/jev/delegate
pub async fn jev_delegate(
    State(state): State<AppState>,
    Json(req): Json<JevDelegateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pipeline = crate::openjev_pipeline::JevDelegationPipeline::new(std::sync::Arc::new(state.clone()));
    match pipeline.decide_model(&req.prompt).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )),
    }
}

// POST /hcs/v2/jev/rate_plan
pub async fn jev_rate_plan(
    State(state): State<AppState>,
    Json(req): Json<JevRatePlanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let contract = crate::openjev_pipeline::JevNormalizer::prepare_plan_rating_contract(&req.goal, &req.plans);
    if let Err(e) = contract.validate() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": { "message": e.to_string() } }))));
    }

    let prompt_rendered = contract.render_prompt();
    let judge_port = match state.ensure_worker("hcs-judge").await {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": { "message": e.to_string() } })))),
    };

    let client = reqwest::Client::new();
    let resp = match client.post(format!("http://127.0.0.1:{}/completion", judge_port))
        .json(&serde_json::json!({
            "prompt": prompt_rendered,
            "temperature": 0.0,
            "n_predict": 4,
            "stop": ["\n", "}", "]"]
        }))
        .send()
        .await {
            Ok(r) => r,
            Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": { "message": e.to_string() } })))),
        };

    let res_json: Value = resp.json().await.unwrap_or_default();
    let raw_output = res_json.get("content").and_then(|v| v.as_str()).unwrap_or("A");
    let decision = contract.parse_output(raw_output);

    Ok(Json(serde_json::json!({
        "goal": req.goal,
        "selected_plan": decision.selected_id,
        "selected_label": decision.selected_label,
        "raw_output": raw_output,
        "decision": decision
    })))
}

// POST /hcs/v2/brain/learn
pub async fn brain_learn(
    State(state): State<AppState>,
    Json(req): Json<BrainLearnRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let conf = req.confidence.unwrap_or(0.95);
    match state.brain.learn(&req.category, &req.pattern, &req.solution, conf) {
        Ok(report) => Ok(Json(serde_json::to_value(report).unwrap())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": { "message": e.to_string() } })))),
    }
}

// GET /hcs/v2/brain/recall
pub async fn brain_recall(
    State(state): State<AppState>,
    Query(params): Query<crate::api::hcs::MemoryQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let q = params.q.unwrap_or_default();
    match state.brain.recall(&q) {
        Ok(records) => Ok(Json(serde_json::json!({ "insights": records, "query": q, "count": records.len() }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": { "message": e.to_string() } })))),
    }
}

// GET /hcs/v2/brain/insights
pub async fn brain_insights(
    State(state): State<AppState>,
    Query(params): Query<crate::api::hcs::MemoryQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = params.limit.unwrap_or(20);
    match state.brain.list_recent(limit) {
        Ok(records) => Ok(Json(serde_json::json!({ "insights": records, "count": records.len() }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": { "message": e.to_string() } })))),
    }
}

// GET /hcs/v2/hardware/profile
pub async fn hardware_profile(State(state): State<AppState>) -> Json<Value> {
    let stats = state.resource.get_stats();
    Json(serde_json::json!({
        "version": "3.0.0",
        "cpu_threads": state.config.resources.threads,
        "batch_size": state.config.resources.batch_size,
        "ubatch_size": state.config.resources.ubatch_size,
        "kv_cache_k": state.config.resources.cache_type_k,
        "kv_cache_v": state.config.resources.cache_type_v,
        "flash_attention": state.config.resources.flash_attention,
        "continuous_batching": true,
        "vulkan_acceleration": true,
        "unified_memory_total_mb": stats.total_memory_mb,
        "unified_memory_used_mb": stats.used_memory_mb,
        "unified_memory_available_mb": stats.available_memory_mb,
        "max_heavy_active": stats.max_heavy_active,
        "active_heavy_count": stats.active_heavy_count,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CompactContextApiRequest {
    pub messages: Vec<serde_json::Value>,
    pub preserve_recent: Option<usize>,
}

// POST /hcs/v2/compact
pub async fn compact_context(
    State(state): State<AppState>,
    Json(req): Json<CompactContextApiRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let compactor = crate::context_compactor::ContextCompactor::new(std::sync::Arc::new(state.clone()));
    let preserve = req.preserve_recent.unwrap_or(2);
    match compactor.compact(&req.messages, preserve).await {
        Ok(res) => Ok(Json(serde_json::to_value(res).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct JSpaceHandoverRequest {
    pub from_model: String,
    pub to_model: String,
    pub context_delta: String,
}

// POST /hcs/v2/jspace/sessions/:id/handover
pub async fn jspace_handover(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
    Json(req): Json<JSpaceHandoverRequest>,
) -> Result<Json<Value>, StatusCode> {
    if state.jspace.handover(&id, &req.from_model, &req.to_model, &req.context_delta) {
        Ok(Json(serde_json::json!({ "success": true, "session_id": id })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Deserialize)]
pub struct SetVariableRequest {
    pub key: String,
    pub value: String,
}

// GET /hcs/v2/jspace/sessions/:id/variables
pub async fn get_jspace_variables(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.jspace.get_variables(&id) {
        Some(vars) => Ok(Json(serde_json::json!({ "variables": vars, "session_id": id }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// POST /hcs/v2/jspace/sessions/:id/variables
pub async fn set_jspace_variable(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
    Json(req): Json<SetVariableRequest>,
) -> Result<Json<Value>, StatusCode> {
    if state.jspace.set_variable(&id, &req.key, &req.value) {
        Ok(Json(serde_json::json!({ "success": true, "session_id": id, "key": req.key })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Deserialize)]
pub struct JSpaceDecideRequest {
    pub state_description: String,
    pub instructions: String,
    pub candidates: Vec<String>,
}

// POST /hcs/v2/jspace/sessions/:id/decide
pub async fn jspace_decide(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
    Json(req): Json<JSpaceDecideRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if state.jspace.get_session(&id).is_none() {
        return Err((StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Session not found" }))));
    }

    if req.candidates.len() < 2 || req.candidates.len() > 16 {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Must provide 2..16 candidates" }))));
    }

    let criteria: Vec<crate::openjev::Candidate> = req.candidates.iter().enumerate().map(|(i, c)| {
        crate::openjev::Candidate {
            id: format!("cand_{}", i + 1),
            description: c.clone(),
        }
    }).collect();

    let decision_req = crate::openjev::OpenJevDecisionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        group_id: format!("jspace-{}", id),
        primitive: "choice".to_string(),
        state: req.state_description.clone(),
        instructions: req.instructions.clone(),
        criteria,
    };

    let prompt = decision_req.render_prompt();
    let judge_port = match state.ensure_worker("hcs-judge").await {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": e.to_string() })))),
    };

    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap_or_default();
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", judge_port);

    let resp = client.post(&url)
        .json(&serde_json::json!({
            "model": "hcs-judge",
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.0,
            "max_tokens": 1
        }))
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))))?;

    let resp_val: Value = resp.json().await.unwrap_or_default();
    let raw_choice = resp_val["choices"][0]["message"]["content"].as_str().unwrap_or("A").trim();
    let parsed = decision_req.parse_output(raw_choice);

    let chosen_id = parsed.candidate_id.as_deref().unwrap_or("cand_1");
    let chosen_desc = req.candidates.get(
        parsed.selected_label
            .and_then(|lbl| (lbl as u8).checked_sub(b'A'))
            .map(|idx| idx as usize)
            .unwrap_or(0)
    ).cloned().unwrap_or_else(|| chosen_id.to_string());

    state.jspace.record_decision(&id, &req.candidates, &chosen_desc, None);

    Ok(Json(serde_json::json!({
        "session_id": id,
        "selected_candidate": chosen_desc,
        "selected_label": parsed.selected_label.map(|c| c.to_string()),
        "response": parsed
    })))
}



