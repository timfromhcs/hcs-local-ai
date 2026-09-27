use crate::openjev::{Candidate, OpenJevDecisionRequest, OpenJevDecisionResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedIntent {
    pub category: String, // "coding", "reasoning", "chat", "multimodal", "image_generation", "tool_use"
    pub complexity: u8,   // 1 to 5
    pub reasoning_depth: String, // "shallow", "moderate", "deep"
    pub candidate_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    pub raw_intent: ExtractedIntent,
    pub prepared_contract: OpenJevDecisionRequest,
    pub decision: OpenJevDecisionResponse,
    pub selected_model: String,
    pub rationale: String,
}

pub struct JevNormalizer;

impl JevNormalizer {
    /// Prepares, sanitizes, and normalizes candidate execution options
    /// strictly according to the OpenJev decision contract (2–16 candidates).
    pub fn prepare_routing_contract(prompt: &str, intent: &ExtractedIntent) -> OpenJevDecisionRequest {
        let mut candidates = Vec::new();

        // Always include Bonsai-2 27B for heavy coding / deep reasoning
        candidates.push(Candidate {
            id: "hcs-coder".to_string(),
            description: "hcs-coder (Bonsai 2-27B PQ2_0): Repository architecture, complex software engineering, deep multi-file refactoring, difficult reasoning, and deep verification".to_string(),
        });

        // Always include Bonsai-4B for standard general reasoning / moderate chat
        candidates.push(Candidate {
            id: "hcs-general".to_string(),
            description: "hcs-general (Ternary Bonsai 4B): Standard chat, explanation, summarization, general questions, and small-to-medium reasoning".to_string(),
        });

        // If multimodal / visual inspection is relevant
        if intent.category == "multimodal" || prompt.to_lowercase().contains("image") || prompt.to_lowercase().contains("picture") || prompt.to_lowercase().contains("screenshot") {
            candidates.push(Candidate {
                id: "hcs-vlm".to_string(),
                description: "hcs-vlm (Qwen3.5 9B Multimodal): Visual inspection, screenshot reading, diagram interpretation, and UI visual reasoning".to_string(),
            });
            candidates.push(Candidate {
                id: "hcs-image".to_string(),
                description: "hcs-image (FLUX.2 Klein): Text-to-Image generation, artistic asset creation, and visual image editing".to_string(),
            });
        } else {
            // Lightweight subagent for quick classifications or short tasks
            candidates.push(Candidate {
                id: "hcs-subagent".to_string(),
                description: "hcs-subagent (Bonsai 1.7B): Quick parsing, small utility transformations, lightweight classification, and sub-second prechecks".to_string(),
            });
        }

        let state = format!(
            "Task Prompt: \"{}\"\nDetected Category: {}\nComplexity Level: {}/5\nReasoning Depth: {}",
            prompt.chars().take(300).collect::<String>(),
            intent.category,
            intent.complexity,
            intent.reasoning_depth
        );

        let instructions = "Evaluate task complexity and domain. If this requires complex coding, deep reasoning, repository manipulation, or extensive tool use, select hcs-coder. If it is standard conversational reasoning, select hcs-general. Otherwise select the smallest capable model.".to_string();

        OpenJevDecisionRequest {
            id: uuid::Uuid::new_v4().to_string(),
            group_id: "model_routing".to_string(),
            primitive: "choice".to_string(),
            state,
            instructions,
            criteria: candidates,
        }
    }

    /// Normalizes and prepares plan candidates for Jev Plan Rating
    pub fn prepare_plan_rating_contract(goal: &str, plans: &[String]) -> OpenJevDecisionRequest {
        let mut criteria = Vec::new();
        for (idx, plan) in plans.iter().enumerate().take(16) {
            criteria.push(Candidate {
                id: format!("plan_{}", idx + 1),
                description: plan.clone(),
            });
        }

        // Must have at least 2 candidates
        if criteria.len() < 2 {
            criteria.push(Candidate {
                id: "plan_abort".to_string(),
                description: "Abort current execution and request further user clarification".to_string(),
            });
        }

        OpenJevDecisionRequest {
            id: uuid::Uuid::new_v4().to_string(),
            group_id: "plan_rating".to_string(),
            primitive: "choice".to_string(),
            state: format!("Goal: {}", goal),
            instructions: "Select the most robust, minimal-risk, and verifiable execution plan for this goal.".to_string(),
            criteria,
        }
    }
}

pub struct JevDelegationPipeline {
    app_state: Arc<crate::api::AppState>,
}

impl JevDelegationPipeline {
    pub fn new(app_state: Arc<crate::api::AppState>) -> Self {
        Self { app_state }
    }

    /// Stage 1: Fast Intent Extraction using Subagent (or fast rules)
    pub async fn extract_intent(&self, prompt: &str) -> ExtractedIntent {
        let lower = prompt.to_lowercase();
        
        let is_coding = lower.contains("code") || lower.contains("function") || lower.contains("class") ||
            lower.contains("struct") || lower.contains("impl") || lower.contains("bug") ||
            lower.contains("compile") || lower.contains("refactor") || lower.contains("rust") ||
            lower.contains("python") || lower.contains("javascript") || lower.contains("git") ||
            lower.contains("api") || lower.contains("error") || lower.contains("test");

        let is_multimodal = lower.contains("image") || lower.contains("photo") || lower.contains("screenshot") ||
            lower.contains("diagram") || lower.contains("ui") || lower.contains("visual");

        let is_heavy_reasoning = lower.contains("prove") || lower.contains("architect") || lower.contains("analyze") ||
            lower.contains("compare in detail") || lower.contains("design") || lower.len() > 1000;

        let category = if is_multimodal {
            "multimodal".to_string()
        } else if is_coding {
            "coding".to_string()
        } else if is_heavy_reasoning {
            "reasoning".to_string()
        } else {
            "chat".to_string()
        };

        let complexity = if is_coding && is_heavy_reasoning {
            5
        } else if is_coding || is_heavy_reasoning {
            4
        } else if prompt.len() > 300 {
            3
        } else {
            2
        };

        let reasoning_depth = match complexity {
            4..=5 => "deep".to_string(),
            3 => "moderate".to_string(),
            _ => "shallow".to_string(),
        };

        let mut candidate_models = Vec::new();
        if category == "coding" || complexity >= 4 {
            candidate_models.push("hcs-coder".to_string());
        }
        candidate_models.push("hcs-general".to_string());
        candidate_models.push("hcs-subagent".to_string());

        ExtractedIntent {
            category,
            complexity,
            reasoning_depth,
            candidate_models,
        }
    }

    /// Complete 3-Stage Pipeline: Extract -> Clean/Prepare -> OpenJev Decide
    pub async fn decide_model(&self, prompt: &str) -> anyhow::Result<DelegationResult> {
        info!("Jev Pipeline Stage 1: Extracting intent for prompt (len: {})...", prompt.len());
        let intent = self.extract_intent(prompt).await;

        info!("Jev Pipeline Stage 2: Normalizing and preparing OpenJev contract...");
        let contract = JevNormalizer::prepare_routing_contract(prompt, &intent);
        contract.validate()?;

        info!("Jev Pipeline Stage 3: Evaluating candidate choice with OpenJev at temp=0...");
        let prompt_rendered = contract.render_prompt();

        let judge_port = self.app_state.ensure_worker("hcs-judge").await?;
        let client = reqwest::Client::new();
        let resp = client.post(format!("http://127.0.0.1:{}/completion", judge_port))
            .json(&serde_json::json!({
                "prompt": prompt_rendered,
                "temperature": 0.0,
                "n_predict": 4,
                "stop": ["\n", "}", "]"]
            }))
            .send()
            .await?;

        let res_json: serde_json::Value = resp.json().await?;
        let raw_output = res_json.get("content").and_then(|v| v.as_str()).unwrap_or("A");

        let decision = contract.parse_output(raw_output);
        let selected_model = decision.selected_id.clone().unwrap_or_else(|| "hcs-general".to_string());

        info!("Jev Decision Complete: Selected label {:?} -> model '{}'", decision.selected_label, selected_model);

        Ok(DelegationResult {
            raw_intent: intent,
            prepared_contract: contract,
            decision,
            selected_model: selected_model.clone(),
            rationale: format!("Deterministic OpenJev selection mapped to {}", selected_model),
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_jev_normalizer_routing_contract() {
        let intent = ExtractedIntent {
            category: "coding".to_string(),
            complexity: 5,
            reasoning_depth: "deep".to_string(),
            candidate_models: vec!["hcs-coder".to_string()],
        };

        let contract = JevNormalizer::prepare_routing_contract(
            "Implement high-performance multi-threaded Vulkan pipeline in Rust",
            &intent,
        );

        assert!(contract.validate().is_ok());
        assert!(contract.criteria.len() >= 3);
        assert_eq!(contract.criteria[0].id, "hcs-coder");
        assert!(contract.criteria[0].description.contains("Bonsai 2-27B"));
    }

    #[test]
    fn test_jev_normalizer_plan_rating_contract() {
        let plans = vec![
            "Plan A: Run atomic multi-file refactor with unit tests".to_string(),
            "Plan B: Edit files sequentially without testing".to_string(),
        ];

        let contract = JevNormalizer::prepare_plan_rating_contract("Refactor parser", &plans);
        assert!(contract.validate().is_ok());
        assert_eq!(contract.criteria.len(), 2);
        assert_eq!(contract.criteria[0].id, "plan_1");
    }
}
