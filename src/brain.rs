use crate::db::{Database, LearnedInsightRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLearnReport {
    pub pattern: String,
    pub solution: String,
    pub category: String,
    pub confidence: f32,
    pub recorded_id: String,
}

pub struct PersistentBrain {
    db: Arc<Database>,
}

impl PersistentBrain {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Records a new learned insight from tool fixes or user solutions
    pub fn learn(&self, category: &str, pattern: &str, solution: &str, confidence: f32) -> anyhow::Result<AutoLearnReport> {
        info!("Brain Auto-Learning: Recording insight under '{}' for pattern '{}'", category, pattern);
        let id = self.db.insert_learned_insight(category, pattern, solution, confidence)?;
        Ok(AutoLearnReport {
            pattern: pattern.to_string(),
            solution: solution.to_string(),
            category: category.to_string(),
            confidence,
            recorded_id: id,
        })
    }

    /// Queries the brain for previously learned solutions matching a problem description
    pub fn recall(&self, query: &str) -> anyhow::Result<Vec<LearnedInsightRecord>> {
        self.db.find_learned_insights(query)
    }

    /// Returns the most recent learned insights
    pub fn list_recent(&self, limit: usize) -> anyhow::Result<Vec<LearnedInsightRecord>> {
        self.db.list_learned_insights(limit)
    }

    /// Automatically learns from a self-healing diagnostic report
    pub fn learn_from_healing(&self, tool: &str, exit_code: i32, error_log: &str, fixed_command: &str) -> anyhow::Result<AutoLearnReport> {
        let pattern = format!("Tool '{}' failed with code {}: {}", tool, exit_code, error_log.chars().take(200).collect::<String>());
        let solution = format!("Use corrected invocation: {}", fixed_command);
        self.learn("self_healing", &pattern, &solution, 0.95)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_brain_learn_and_recall() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let brain = PersistentBrain::new(db);

        let report = brain.learn("vulkan_tuning", "AMD iGPU memory pressure", "Apply Q8 KV cache and threads 8", 0.98).unwrap();
        assert_eq!(report.category, "vulkan_tuning");

        let recalled = brain.recall("iGPU").unwrap();
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].solution, "Apply Q8 KV cache and threads 8");

        let healing_report = brain.learn_from_healing("cargo_build", 1, "missing semicolon", "cargo fix --bin").unwrap();
        assert_eq!(healing_report.category, "self_healing");
        assert!(healing_report.solution.contains("cargo fix"));
    }
}
