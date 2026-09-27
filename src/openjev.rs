use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PROMPT_VERSION: &str = "jev.dynamic.prompt.v2";
pub const LABELS: [char; 16] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H',
    'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P',
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenJevDecisionRequest {
    pub id: String,
    #[serde(default = "default_group_id")]
    pub group_id: String,
    #[serde(default = "default_primitive")]
    pub primitive: String, // "choice", "noul", "score_level"
    pub state: String,
    pub instructions: String,
    pub criteria: Vec<Candidate>,
}

fn default_group_id() -> String {
    "default".to_string()
}
fn default_primitive() -> String {
    "choice".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenJevDecisionResponse {
    pub id: String,
    pub r#type: String,
    pub choice: Option<String>,
    pub selected_label: Option<char>,
    pub candidate_id: Option<String>,
    pub probabilities: HashMap<String, f32>,
    pub raw_response: String,
}

impl OpenJevDecisionRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.state.trim().is_empty() {
            anyhow::bail!("state must be a nonempty string");
        }
        if self.instructions.trim().is_empty() {
            anyhow::bail!("instructions must be a nonempty string");
        }
        if self.criteria.len() < 2 || self.criteria.len() > 16 {
            anyhow::bail!("criteria must contain 2..16 candidates, found {}", self.criteria.len());
        }
        let mut ids = std::collections::HashSet::new();
        for c in &self.criteria {
            if c.id.trim().is_empty() {
                anyhow::bail!("candidate id must be nonempty");
            }
            if c.description.trim().is_empty() {
                anyhow::bail!("candidate description must be nonempty");
            }
            if !ids.insert(&c.id) {
                anyhow::bail!("duplicate candidate id: {}", c.id);
            }
        }
        Ok(())
    }

    pub fn label_mapping(&self) -> HashMap<char, String> {
        let mut map = HashMap::new();
        for (i, c) in self.criteria.iter().enumerate() {
            map.insert(LABELS[i], c.id.clone());
        }
        map
    }

    pub fn render_prompt(&self) -> String {
        let prefix = format!("Shared state:\n{}\n\n", self.state);

        let criteria_json: Vec<serde_json::Value> = self.criteria.iter().enumerate().map(|(i, c)| {
            serde_json::json!({
                "label": LABELS[i].to_string(),
                "description": c.description
            })
        }).collect();

        let task = serde_json::json!({
            "criteria": criteria_json,
            "instructions": self.instructions,
            "primitive": self.primitive,
        });

        let suffix_json = serde_json::to_string(&task).unwrap_or_default();
        let allowed_labels: Vec<String> = LABELS[..self.criteria.len()].iter().map(|c| c.to_string()).collect();

        format!(
            "{}{}\nReturn only the selected letter: {}.\nAnswer:",
            prefix,
            suffix_json,
            allowed_labels.join(", ")
        )
    }

    pub fn parse_output(&self, raw_output: &str) -> OpenJevDecisionResponse {
        let mapping = self.label_mapping();
        let trimmed = raw_output.trim();

        // Extract first matching uppercase label in the output
        let mut found_label = None;
        for ch in trimmed.chars() {
            let upper = ch.to_ascii_uppercase();
            if mapping.contains_key(&upper) {
                found_label = Some(upper);
                break;
            }
        }

        let mut probs = HashMap::new();
        let candidate_count = self.criteria.len() as f32;
        let uniform = 1.0 / candidate_count;

        for c in &self.criteria {
            probs.insert(c.id.clone(), uniform);
        }

        let (choice, candidate_id) = if let Some(lbl) = found_label {
            if let Some(id) = mapping.get(&lbl) {
                probs.insert(id.clone(), 0.95);
                // Renormalize others
                let rem = 0.05 / (candidate_count - 1.0).max(1.0);
                for c in &self.criteria {
                    if c.id != *id {
                        probs.insert(c.id.clone(), rem);
                    }
                }
                (Some(id.clone()), Some(id.clone()))
            } else {
                (None, None)
            }
        } else {
            // Default to first candidate if unparseable
            let first = self.criteria[0].id.clone();
            (Some(first.clone()), Some(first))
        };

        OpenJevDecisionResponse {
            id: self.id.clone(),
            r#type: self.primitive.clone(),
            choice: choice.clone(),
            selected_label: found_label,
            candidate_id,
            probabilities: probs,
            raw_response: raw_output.to_string(),
        }
    }
}
