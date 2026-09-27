use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttempt {
    pub iteration: u32,
    pub diagnosis: String,
    pub proposed_fix: String,
    pub applied_tool: String,
    pub test_passed: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHealingReport {
    pub task_id: String,
    pub initial_error: String,
    pub attempts: Vec<RepairAttempt>,
    pub resolved: bool,
    pub final_summary: String,
}

pub struct SelfHealingEngine {
    #[allow(dead_code)]
    pub max_iterations: u32,
}

impl SelfHealingEngine {
    pub fn new(max_iterations: u32) -> Self {
        Self { max_iterations }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn create_report(&self, task_id: &str, initial_error: &str) -> SelfHealingReport {
        SelfHealingReport {
            task_id: task_id.to_string(),
            initial_error: initial_error.to_string(),
            attempts: Vec::new(),
            resolved: false,
            final_summary: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn record_attempt(
        &self,
        report: &mut SelfHealingReport,
        diagnosis: &str,
        proposed_fix: &str,
        applied_tool: &str,
        passed: bool,
    ) {
        let iteration = report.attempts.len() as u32 + 1;
        report.attempts.push(RepairAttempt {
            iteration,
            diagnosis: diagnosis.to_string(),
            proposed_fix: proposed_fix.to_string(),
            applied_tool: applied_tool.to_string(),
            test_passed: passed,
        });
        if passed {
            report.resolved = true;
            report.final_summary = format!("Resolved in iteration {}", iteration);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_healing_diagnosis_and_reporting() {
        let engine = SelfHealingEngine::new(3);
        assert!(engine.can_attempt(0));
        assert!(engine.can_attempt(2));
        assert!(!engine.can_attempt(3));

        let diag_rust = engine.diagnose_failure("error[E0308]: mismatched types");
        assert!(diag_rust.contains("Rust compiler error"));

        let diag_file = engine.diagnose_failure("cannot find file foo.txt");
        assert!(diag_file.contains("Missing file"));

        let mut report = engine.create_report("task-1", "compile error");
        assert!(!report.resolved);

        engine.record_attempt(&mut report, &diag_rust, "Cast integer", "file_edit", true);
        assert!(report.resolved);
        assert_eq!(report.attempts.len(), 1);
    }
}
