use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttempt {
    pub iteration: u32,
    pub diagnosis: String,
    pub proposed_fix: String,
    pub applied_tool: String,
    pub test_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHealingReport {
    pub task_id: String,
    pub initial_error: String,
    pub attempts: Vec<RepairAttempt>,
    pub resolved: bool,
    pub final_summary: String,
}

pub struct SelfHealingEngine {
    pub max_iterations: u32,
}

impl SelfHealingEngine {
    pub fn new(max_iterations: u32) -> Self {
        Self { max_iterations }
    }

    pub fn can_attempt(&self, current_attempt: u32) -> bool {
        current_attempt < self.max_iterations
    }

    pub fn diagnose_failure(&self, error_output: &str) -> String {
        if error_output.contains("error[E") {
            "Rust compiler error detected. Inspecting types, syntax, or lifetime errors.".to_string()
        } else if error_output.contains("cannot find file") || error_output.contains("No such file") {
            "Missing file or path discrepancy detected.".to_string()
        } else if error_output.contains("test failed") || error_output.contains("FAILED") {
            "Unit/Integration test assertion failure detected.".to_string()
        } else {
            format!("General failure: {}", error_output.chars().take(200).collect::<String>())
        }
    }
}
