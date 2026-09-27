pub mod tools;
pub mod self_healing;

pub use tools::{ToolCall, ToolExecutor, ToolResult};
pub use self_healing::SelfHealingEngine;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub description: String,
    pub status: String, // "planning", "running", "completed", "failed"
    pub steps_taken: Vec<AgentStep>,
    pub result: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_index: usize,
    pub thought: String,
    pub tool_call: Option<ToolCall>,
    pub tool_result: Option<ToolResult>,
}

pub struct AgentRuntime {
    pub executor: Arc<ToolExecutor>,
    pub self_healing: SelfHealingEngine,
}

impl AgentRuntime {
    pub fn new(executor: Arc<ToolExecutor>) -> Self {
        Self {
            executor,
            self_healing: SelfHealingEngine::new(3),
        }
    }

    pub fn create_task(&self, description: &str) -> AgentTask {
        AgentTask {
            id: Uuid::new_v4().to_string(),
            description: description.to_string(),
            status: "planning".to_string(),
            steps_taken: Vec::new(),
            result: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}
