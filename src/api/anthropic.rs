use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

use crate::api::AppState;
use crate::db::RequestTraceRecord;

#[derive(Debug, Deserialize)]
pub struct AnthropicMessageRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub system: Option<Value>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub tools: Option<Vec<Value>>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Value, // string or array of blocks
}

fn default_max_tokens() -> u32 {
    2048
}

// POST /v1/messages
pub async fn create_message(
    State(state): State<AppState>,
    Json(payload): Json<AnthropicMessageRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let start_time = Instant::now();
    let target_model_id = state.registry.resolve_alias(&payload.model);

    let port = state.ensure_worker(&target_model_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": format!("Failed to ensure worker for {}: {}", target_model_id, e)
                }
            })),
        )
    })?;

    // Convert Anthropic messages to OpenAI format
    let mut openai_messages = Vec::new();

    if let Some(sys) = payload.system {
        let sys_content = if let Some(s) = sys.as_str() {
            s.to_string()
        } else if let Some(arr) = sys.as_array() {
            arr.iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };
        if !sys_content.is_empty() {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": sys_content
            }));
        }
    }

    for msg in payload.messages {
        let role = msg.role;
        let content = if let Some(s) = msg.content.as_str() {
            serde_json::json!(s)
        } else if let Some(blocks) = msg.content.as_array() {
            let mut text_parts = Vec::new();
            for block in blocks {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(t);
                }
            }
            serde_json::json!(text_parts.join("\n"))
        } else {
            msg.content
        };

        openai_messages.push(serde_json::json!({
            "role": role,
            "content": content
        }));
    }

    let forward_body = serde_json::json!({
        "model": target_model_id,
        "messages": openai_messages,
        "max_tokens": payload.max_tokens,
        "temperature": payload.temperature.unwrap_or(0.7),
        "stream": payload.stream,
        "tools": payload.tools,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_default();
    let worker_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let resp = client.post(&worker_url)
        .json(&forward_body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "type": "error",
                    "error": { "type": "api_error", "message": e.to_string() }
                })),
            )
        })?;

    let message_id = format!("msg_{}", Uuid::new_v4().to_string().replace('-', ""));

    if payload.stream {
        // SSE translation into Anthropic SSE events
        let stream = resp.bytes_stream().map(move |item| {
            match item {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let mut events = Vec::new();
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let json_str = &line[6..].trim();
                            if *json_str == "[DONE]" {
                                events.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string());
                            } else if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                if let Some(delta) = parsed.get("choices").and_then(|c| c.get(0)).and_then(|c0| c0.get("delta")) {
                                    if let Some(content) = delta.get("content").and_then(|t| t.as_str()) {
                                        let ev = serde_json::json!({
                                            "type": "content_block_delta",
                                            "index": 0,
                                            "delta": { "type": "text_delta", "text": content }
                                        });
                                        events.push(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&ev).unwrap()));
                                    }
                                }
                            }
                        }
                    }
                    Ok(Bytes::from(events.join("")))
                }
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
            }
        });

        Ok(Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .body(axum::body::Body::from_stream(stream))
            .unwrap())
    } else {
        let latency_ms = start_time.elapsed().as_millis() as u64;
        let worker_json: Value = resp.json().await.unwrap_or_default();

        let choice = worker_json.get("choices").and_then(|c| c.get(0));
        let text_content = choice
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .unwrap_or("");

        let usage = worker_json.get("usage");
        let prompt_tokens = usage.and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let completion_tokens = usage.and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        let trace = RequestTraceRecord {
            id: message_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            endpoint: "/v1/messages".to_string(),
            model_requested: payload.model.clone(),
            model_used: target_model_id.clone(),
            prompt_tokens,
            completion_tokens,
            latency_ms,
            status_code: 200,
            client_ip: "127.0.0.1".to_string(),
            trace_data: format!("anthropic_tokens:{}+{}", prompt_tokens, completion_tokens),
        };
        let _ = state.db.record_request(&trace);
        state.telemetry.record_request(&target_model_id, prompt_tokens, completion_tokens, latency_ms, false);
        state.broadcast_event("request", &trace);

        let anthropic_resp = serde_json::json!({
            "id": message_id,
            "type": "message",
            "role": "assistant",
            "model": target_model_id,
            "content": [
                {
                    "type": "text",
                    "text": text_content
                }
            ],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": prompt_tokens,
                "output_tokens": completion_tokens
            }
        });

        Ok(Json(anthropic_resp).into_response())
    }
}

// POST /v1/messages/count_tokens
pub async fn count_tokens(Json(payload): Json<Value>) -> Json<Value> {
    let mut total_chars = 0;
    if let Some(messages) = payload.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                total_chars += content.len();
            }
        }
    }
    let estimated_tokens = (total_chars / 4).max(1);
    Json(serde_json::json!({ "input_tokens": estimated_tokens }))
}
