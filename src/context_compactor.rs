use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use crate::api::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContextRequest {
    pub messages: Vec<Value>,
    pub target_model: Option<String>,
    pub max_tokens_target: Option<usize>,
    pub preserve_recent_turns: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContextResponse {
    pub compacted_messages: Vec<Value>,
    pub original_tokens_est: usize,
    pub compacted_tokens_est: usize,
    pub compression_ratio: f32,
    pub context_delta: Option<String>,
}

pub struct ContextCompactor {
    state: Arc<AppState>,
}

impl ContextCompactor {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Estimate token count using word/char heuristic (~3.8 chars per token).
    pub fn estimate_tokens(messages: &[Value]) -> usize {
        let mut chars = 0;
        for m in messages {
            if let Some(content) = m.get("content").and_then(|c| c.as_str()) {
                chars += content.len();
            }
        }
        (chars as f32 / 3.8).ceil() as usize
    }

    /// Prune verbose tool calls, test traces, or repetitious shell dumps.
    pub fn extractive_prune(messages: &[Value]) -> Vec<Value> {
        let mut pruned = Vec::new();
        for m in messages {
            let mut cloned = m.clone();
            if let Some(content) = cloned.get("content").and_then(|c| c.as_str()) {
                // If content is huge terminal output or traceback > 1500 chars, prune middle
                if content.len() > 1500 && (content.contains("test session starts") || content.contains("Traceback (most recent call last)")) {
                    let head: String = content.chars().take(400).collect();
                    let tail: String = content.chars().skip(content.len().saturating_sub(600)).collect();
                    let compressed = format!("{}\n\n[... HCS Extractive Pruner: Truncated {} chars of terminal log ...]\n\n{}", head, content.len() - 1000, tail);
                    if let Some(obj) = cloned.as_object_mut() {
                        obj.insert("content".to_string(), Value::String(compressed));
                    }
                }
            }
            pruned.push(cloned);
        }
        pruned
    }

    /// Compact a message list using hcs-subagent (Bonsai 1.7B).
    pub async fn compact(
        &self,
        messages: &[Value],
        preserve_recent: usize,
    ) -> anyhow::Result<CompactContextResponse> {
        let pruned = Self::extractive_prune(messages);
        let orig_est = Self::estimate_tokens(&pruned);
        if pruned.len() <= preserve_recent + 1 || orig_est < 2000 {
            // Already compact enough
            return Ok(CompactContextResponse {
                compacted_messages: pruned,
                original_tokens_est: orig_est,
                compacted_tokens_est: orig_est,
                compression_ratio: 1.0,
                context_delta: None,
            });
        }

        info!("Triggering Context Compaction (Original tokens est: {})...", orig_est);

        // Separate system message (turn 0), older turns to compact, and recent turns to preserve
        let mut system_msg: Option<Value> = None;
        let mut to_compact: Vec<Value> = Vec::new();
        let mut preserved: Vec<Value> = Vec::new();

        let total = pruned.len();
        let split_idx = total.saturating_sub(preserve_recent.max(2));

        for (i, m) in pruned.iter().enumerate() {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");

            if i == 0 && role == "system" {
                system_msg = Some(m.clone());
            } else if i >= split_idx {
                preserved.push(m.clone());
            } else {
                to_compact.push(m.clone());
            }
        }

        // Format historical turns into text for hcs-subagent distillation
        let mut hist_text = String::new();
        for m in &to_compact {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            hist_text.push_str(&format!("{}: {}\n", role.to_uppercase(), content));
        }

        let prompt = format!(
            "You are the HCS Context Compactor. Distill the following historical conversation turns into a structured Context Delta.\n\
             Extract strictly:\n\
             # ACTIVE GOAL: (Core user goal)\n\
             # SYSTEM STATE: (Current status, modified files, test results)\n\
             # KEY DISCOVERIES: (Important findings, constraints, or decisions)\n\
             # PRESERVED CODE: (Key functions or lines currently under work)\n\n\
             Conversation to summarize:\n{}",
            hist_text
        );

        // Query hcs-subagent (Bonsai 1.7B)
        let subagent_port = self.state.ensure_worker("hcs-subagent").await?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()?;

        let subagent_resp = client
            .post(format!("http://127.0.0.1:{}/v1/chat/completions", subagent_port))
            .json(&serde_json::json!({
                "model": "hcs-subagent",
                "messages": [
                    {"role": "system", "content": "You are a concise context synthesis engine. Summarize technical state accurately without filler words."},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.1,
                "max_tokens": 400
            }))
            .send()
            .await?;

        let resp_json: Value = subagent_resp.json().await.unwrap_or_default();
        let context_delta = resp_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .unwrap_or("# ACTIVE GOAL: Continuing session.\n# SYSTEM STATE: Historical context preserved.")
            .to_string();

        // Assemble compacted messages
        let mut final_msgs: Vec<Value> = Vec::new();
        if let Some(sys) = system_msg {
            final_msgs.push(sys);
        }

        // Insert synthesized context delta turn
        final_msgs.push(serde_json::json!({
            "role": "system",
            "content": format!("[HCS COMPACTED CONTEXT DELTA]\n{}", context_delta)
        }));

        // Append preserved recent turns
        final_msgs.extend(preserved);

        let final_est = Self::estimate_tokens(&final_msgs);
        let ratio = if orig_est > 0 { final_est as f32 / orig_est as f32 } else { 1.0 };

        info!("Compaction complete: {} tokens -> {} tokens (ratio: {:.2})", orig_est, final_est, ratio);

        Ok(CompactContextResponse {
            compacted_messages: final_msgs,
            original_tokens_est: orig_est,
            compacted_tokens_est: final_est,
            compression_ratio: ratio,
            context_delta: Some(context_delta),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_and_prune() {
        let msgs = vec![
            serde_json::json!({ "role": "user", "content": "Short prompt here." }),
            serde_json::json!({
                "role": "system",
                "content": format!("Traceback (most recent call last):\n{}", "a".repeat(2000))
            }),
        ];

        let est = ContextCompactor::estimate_tokens(&msgs);
        assert!(est > 500);

        let pruned = ContextCompactor::extractive_prune(&msgs);
        assert_eq!(pruned.len(), 2);
        let pruned_traceback = pruned[1]["content"].as_str().unwrap();
        assert!(pruned_traceback.contains("HCS Extractive Pruner"));
        assert!(pruned_traceback.len() < 2000);
    }
}

