use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

use crate::api::AppState;
use crate::backend::sd::ImageGenerateParams;
use crate::db::RequestTraceRecord;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Value>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<Value>>,
    pub tool_choice: Option<Value>,
    pub response_format: Option<Value>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct ImageGenerateRequest {
    pub prompt: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_size")]
    pub size: String,
    #[serde(default = "default_steps")]
    pub steps: Option<u32>,
    #[serde(default)]
    pub response_format: Option<String>,
}

fn default_model() -> String {
    "hcs-image".to_string()
}
fn default_size() -> String {
    "1024x1024".to_string()
}
fn default_steps() -> Option<u32> {
    Some(4)
}

// GET /v1/models
pub async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let models = state.registry.list_models();
    let data: Vec<Value> = models.into_iter().map(|m| {
        serde_json::json!({
            "id": m.manifest.id,
            "object": "model",
            "created": 1727460000,
            "owned_by": "hcs-local-ai",
            "capabilities": m.manifest.capabilities,
            "state": m.state,
            "context_length": m.manifest.context,
            "quantization": m.manifest.quantization,
        })
    }).collect();

    Json(serde_json::json!({
        "object": "list",
        "data": data,
    }))
}

// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<ChatCompletionRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let start_time = Instant::now();
    let target_model_id = state.registry.resolve_alias(&payload.model);
    let is_streaming = payload.stream;

    // Check if worker is ready, or start it
    let port = state.ensure_worker(&target_model_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "message": format!("Failed to initialize worker for {}: {}", target_model_id, e),
                    "type": "server_error"
                }
            })),
        )
    })?;

    // Prepare payload forwarded to llama-server
    payload.model = target_model_id.clone();
    let forward_val = serde_json::to_value(&payload).unwrap();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_default();
    let worker_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let resp = client.post(&worker_url)
        .json(&forward_val)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {
                        "message": format!("Worker request failed: {}", e),
                        "type": "bad_gateway"
                    }
                })),
            )
        })?;

    let client_ip = headers.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1")
        .to_string();

    if is_streaming {
        let stream = resp.bytes_stream().map(|item| item.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
        
        let trace = RequestTraceRecord {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            endpoint: "/v1/chat/completions".to_string(),
            model_requested: payload.model.clone(),
            model_used: target_model_id.clone(),
            prompt_tokens: 0,
            completion_tokens: 0,
            latency_ms: start_time.elapsed().as_millis() as u64,
            status_code: 200,
            client_ip,
            trace_data: "stream:true".to_string(),
        };
        let _ = state.db.record_request(&trace);
        state.telemetry.record_request(&target_model_id, 0, 0, trace.latency_ms, false);
        state.broadcast_event("request", &trace);

        Ok(Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(axum::body::Body::from_stream(stream))
            .unwrap())
    } else {
        let status = resp.status();
        let body_bytes = resp.bytes().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": { "message": format!("Failed reading worker response: {}", e) }
                })),
            )
        })?;

        let latency_ms = start_time.elapsed().as_millis() as u64;
        let mut prompt_tokens = 0;
        let mut completion_tokens = 0;

        if let Ok(json_resp) = serde_json::from_slice::<Value>(&body_bytes) {
            if let Some(usage) = json_resp.get("usage") {
                prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            }
        }

        let trace = RequestTraceRecord {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            endpoint: "/v1/chat/completions".to_string(),
            model_requested: payload.model.clone(),
            model_used: target_model_id.clone(),
            prompt_tokens,
            completion_tokens,
            latency_ms,
            status_code: status.as_u16(),
            client_ip,
            trace_data: format!("tokens:{}+{}", prompt_tokens, completion_tokens),
        };
        let _ = state.db.record_request(&trace);
        state.telemetry.record_request(&target_model_id, prompt_tokens, completion_tokens, latency_ms, !status.is_success());
        state.broadcast_event("request", &trace);

        Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body_bytes))
            .unwrap())
    }
}

// POST /v1/completions
pub async fn completions(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let requested_model = payload.get("model").and_then(|v| v.as_str()).unwrap_or("hcs-general");
    let target_model_id = state.registry.resolve_alias(requested_model);

    let port = state.ensure_worker(&target_model_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    let client = reqwest::Client::new();
    let worker_url = format!("http://127.0.0.1:{}/completion", port);

    let resp = client.post(&worker_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": { "message": e.to_string() } })),
            )
        })?;

    let status = resp.status();
    let bytes = resp.bytes().await.unwrap_or_default();
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(bytes))
        .unwrap())
}

// POST /v1/responses (Agentic responses API)
pub async fn responses(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prompt = payload.get("input").and_then(|v| v.as_str())
        .or_else(|| payload.get("prompt").and_then(|v| v.as_str()))
        .unwrap_or("Hello");

    let task = state.agent.create_task(prompt);
    let target_model = "hcs-general";

    let port = state.ensure_worker(target_model).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    let client = reqwest::Client::new();
    let worker_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let req_body = serde_json::json!({
        "model": target_model,
        "messages": [
            { "role": "system", "content": "You are HCS Local AI Agent. Provide accurate and direct assistance." },
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.2
    });

    let resp = client.post(&worker_url)
        .json(&req_body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": { "message": e.to_string() } })),
            )
        })?;

    let worker_json: Value = resp.json().await.unwrap_or_default();
    let content = worker_json.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .unwrap_or("Completed.");

    Ok(Json(serde_json::json!({
        "id": format!("resp_{}", task.id),
        "object": "response",
        "status": "completed",
        "output": content,
        "task_id": task.id,
        "usage": worker_json.get("usage").cloned().unwrap_or(serde_json::json!({
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }))
    })))
}

// POST /v1/images/generations
pub async fn images_generations(
    State(state): State<AppState>,
    Json(payload): Json<ImageGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let start_time = Instant::now();
    let (width, height) = match payload.size.as_str() {
        "512x512" => (512, 512),
        "768x768" => (768, 768),
        _ => (1024, 1024),
    };

    let params = ImageGenerateParams {
        prompt: payload.prompt.clone(),
        width,
        height,
        steps: payload.steps.unwrap_or(4),
        seed: 42,
        ref_image_path: None,
    };

    let img_path = state.sd_worker.generate_image(params).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": { "message": format!("stable-diffusion.cpp generation failed: {}", e) }
            })),
        )
    })?;

    let filename = img_path.file_name().and_then(|n| n.to_str()).unwrap_or("output.png").to_string();
    let file_url = format!("http://{}:{}/artifacts/{}", state.config.server.host, state.config.server.port, filename);

    // Save artifact to db
    if let Ok(metadata) = std::fs::metadata(&img_path) {
        let _ = state.db.save_artifact(
            &Uuid::new_v4().to_string(),
            &filename,
            "image/png",
            metadata.len(),
            &serde_json::json!({ "prompt": payload.prompt, "size": payload.size }).to_string(),
        );
    }

    let latency_ms = start_time.elapsed().as_millis() as u64;
    let trace = RequestTraceRecord {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        endpoint: "/v1/images/generations".to_string(),
        model_requested: "hcs-image".to_string(),
        model_used: "hcs-image".to_string(),
        prompt_tokens: 0,
        completion_tokens: 0,
        latency_ms,
        status_code: 200,
        client_ip: "127.0.0.1".to_string(),
        trace_data: format!("image:{}", filename),
    };
    let _ = state.db.record_request(&trace);
    state.telemetry.record_request("hcs-image", 0, 0, latency_ms, false);
    state.broadcast_event("image_generation", &trace);

    let return_b64 = payload.response_format.as_deref() == Some("b64_json");
    let item = if return_b64 {
        let bytes = std::fs::read(&img_path).unwrap_or_default();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
        serde_json::json!({ "b64_json": b64, "url": file_url })
    } else {
        serde_json::json!({ "url": file_url })
    };

    Ok(Json(serde_json::json!({
        "created": chrono::Utc::now().timestamp(),
        "data": [item]
    })))
}

// POST /v1/images/edits
pub async fn images_edits(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut prompt = String::new();
    let mut ref_image_path = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "prompt" {
            prompt = field.text().await.unwrap_or_default();
        } else if name == "image" {
            let data = field.bytes().await.unwrap_or_default();
            let temp_ref = state.config.storage.artifacts_dir.join(format!("ref_{}.png", Uuid::new_v4()));
            if tokio::fs::write(&temp_ref, &data).await.is_ok() {
                ref_image_path = Some(temp_ref);
            }
        }
    }

    if prompt.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": { "message": "Missing prompt field" } })),
        ));
    }

    let params = ImageGenerateParams {
        prompt,
        width: 1024,
        height: 1024,
        steps: 4,
        seed: 42,
        ref_image_path,
    };

    let img_path = state.sd_worker.generate_image(params).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": { "message": format!("stable-diffusion.cpp image edit failed: {}", e) }
            })),
        )
    })?;

    let filename = img_path.file_name().and_then(|n| n.to_str()).unwrap_or("edit.png").to_string();
    let file_url = format!("http://{}:{}/artifacts/{}", state.config.server.host, state.config.server.port, filename);

    Ok(Json(serde_json::json!({
        "created": chrono::Utc::now().timestamp(),
        "data": [{ "url": file_url }]
    })))
}

// Files API
pub async fn list_files(State(state): State<AppState>) -> Json<Value> {
    let files = state.db.list_artifacts().unwrap_or_default();
    let data: Vec<Value> = files.into_iter().map(|f| {
        serde_json::json!({
            "id": f.id,
            "object": "file",
            "bytes": f.size_bytes,
            "created_at": 1727460000,
            "filename": f.filename,
            "purpose": "assistants",
        })
    }).collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}

pub async fn get_file(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Ok(Some(f)) = state.db.get_artifact(&id) {
        Ok(Json(serde_json::json!({
            "id": f.id,
            "object": "file",
            "bytes": f.size_bytes,
            "created_at": 1727460000,
            "filename": f.filename,
            "purpose": "assistants",
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": { "message": "File not found" } })),
        ))
    }
}

pub async fn delete_file(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if state.db.delete_artifact(&id).unwrap_or(false) {
        Ok(Json(serde_json::json!({
            "id": id,
            "object": "file",
            "deleted": true
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": { "message": "File not found" } })),
        ))
    }
}

// Batches API
pub async fn create_batch(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let job = state.db.create_job("batch", &payload.to_string()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": e.to_string() } })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "id": job.id,
        "object": "batch",
        "status": job.status,
        "created_at": chrono::Utc::now().timestamp(),
    })))
}

pub async fn get_batch(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Ok(Some(job)) = state.db.get_job(&id) {
        Ok(Json(serde_json::json!({
            "id": job.id,
            "object": "batch",
            "status": job.status,
            "created_at": chrono::Utc::now().timestamp(),
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": { "message": "Batch not found" } })),
        ))
    }
}
