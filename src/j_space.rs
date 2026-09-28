use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSpaceTurn {
    pub role: String,
    pub model: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_results: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSpaceSession {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub goals: Vec<String>,
    pub shared_state: HashMap<String, String>,
    pub active_artifacts: Vec<String>,
    pub turns: Vec<JSpaceTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSpaceSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub turn_count: usize,
    pub goal_count: usize,
    pub state_keys: Vec<String>,
}

pub struct JSpaceManager {
    sessions: Arc<RwLock<HashMap<String, JSpaceSession>>>,
    max_sessions: usize,
}

impl JSpaceManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_sessions,
        }
    }

    pub fn create_session(&self, title: Option<String>) -> JSpaceSession {
        let mut map = self.sessions.write().unwrap();

        // Evict oldest session if at limit
        if map.len() >= self.max_sessions {
            if let Some(oldest_id) = map.iter()
                .min_by_key(|(_, s)| s.last_accessed_at)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest_id);
            }
        }

        let now = Utc::now();
        let id = format!("jspace-{}", uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>());
        let session = JSpaceSession {
            id: id.clone(),
            title: title.unwrap_or_else(|| format!("Session {}", id)),
            created_at: now,
            last_accessed_at: now,
            goals: Vec::new(),
            shared_state: HashMap::new(),
            active_artifacts: Vec::new(),
            turns: Vec::new(),
        };

        map.insert(id, session.clone());
        session
    }

    pub fn get_session(&self, id: &str) -> Option<JSpaceSession> {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(id) {
            session.last_accessed_at = Utc::now();
            Some(session.clone())
        } else {
            None
        }
    }

    pub fn list_sessions(&self) -> Vec<JSpaceSessionSummary> {
        let map = self.sessions.read().unwrap();
        let mut summaries: Vec<JSpaceSessionSummary> = map.values().map(|s| JSpaceSessionSummary {
            id: s.id.clone(),
            title: s.title.clone(),
            created_at: s.created_at,
            last_accessed_at: s.last_accessed_at,
            turn_count: s.turns.len(),
            goal_count: s.goals.len(),
            state_keys: s.shared_state.keys().cloned().collect(),
        }).collect();
        summaries.sort_by(|a, b| b.last_accessed_at.cmp(&a.last_accessed_at));
        summaries
    }

    pub fn set_shared_state(&self, session_id: &str, key: &str, value: &str) -> bool {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(session_id) {
            session.shared_state.insert(key.to_string(), value.to_string());
            session.last_accessed_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn add_goal(&self, session_id: &str, goal: &str) -> bool {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(session_id) {
            session.goals.push(goal.to_string());
            session.last_accessed_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn append_turn(&self, session_id: &str, turn: JSpaceTurn) -> bool {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(session_id) {
            session.turns.push(turn);
            session.last_accessed_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn get_variables(&self, session_id: &str) -> Option<HashMap<String, String>> {
        let map = self.sessions.read().unwrap();
        map.get(session_id).map(|s| s.shared_state.clone())
    }

    pub fn set_variable(&self, session_id: &str, key: &str, value: &str) -> bool {
        self.set_shared_state(session_id, key, value)
    }

    pub fn handover(&self, session_id: &str, from_model: &str, to_model: &str, context_delta: &str) -> bool {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(session_id) {
            let turn = JSpaceTurn {
                role: "system".to_string(),
                model: from_model.to_string(),
                content: format!("[J-SPACE HANDOVER: {} -> {}]\n{}", from_model, to_model, context_delta),
                tool_calls: None,
                tool_results: None,
                timestamp: Utc::now(),
            };
            session.turns.push(turn);
            session.shared_state.insert("active_model".to_string(), to_model.to_string());
            session.shared_state.insert("last_handover".to_string(), Utc::now().to_rfc3339());
            session.last_accessed_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        let mut map = self.sessions.write().unwrap();
        map.remove(session_id).is_some()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_jspace_session_lifecycle() {
        let mgr = JSpaceManager::new(10);
        let s = mgr.create_session(Some("Test Space".to_string()));
        assert_eq!(s.title, "Test Space");

        // Add goal and shared state
        assert!(mgr.add_goal(&s.id, "Achieve 100% test passing"));
        assert!(mgr.set_shared_state(&s.id, "active_model", "bonsai-2-27b"));

        // Append turn
        let turn = JSpaceTurn {
            role: "user".to_string(),
            model: "hcs-coder".to_string(),
            content: "Write a high-performance Vulkan kernel".to_string(),
            tool_calls: None,
            tool_results: None,
            timestamp: Utc::now(),
        };
        assert!(mgr.append_turn(&s.id, turn));

        // Retrieve and inspect
        let fetched = mgr.get_session(&s.id).unwrap();
        assert_eq!(fetched.goals.len(), 1);
        assert_eq!(fetched.shared_state.get("active_model").unwrap(), "bonsai-2-27b");
        assert_eq!(fetched.turns.len(), 1);

        // List
        let list = mgr.list_sessions();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, s.id);

        // Delete
        assert!(mgr.delete_session(&s.id));
        assert!(mgr.get_session(&s.id).is_none());
    }

    #[test]
    fn test_jspace_handover_and_variables() {
        let mgr = JSpaceManager::new(5);
        let s = mgr.create_session(Some("Multi-Agent Session".to_string()));

        // Set variables
        assert!(mgr.set_variable(&s.id, "active_repo", "/workspace/repo"));
        assert!(mgr.set_variable(&s.id, "git_branch", "feat/v3"));

        let vars = mgr.get_variables(&s.id).unwrap();
        assert_eq!(vars.get("active_repo").unwrap(), "/workspace/repo");
        assert_eq!(vars.get("git_branch").unwrap(), "feat/v3");

        // Handover from subagent to coder
        let ok = mgr.handover(
            &s.id,
            "hcs-subagent",
            "hcs-coder",
            "# ACTIVE GOAL: Implement AST parser\n# STATE: 1 file modified",
        );
        assert!(ok);

        let s_after = mgr.get_session(&s.id).unwrap();
        assert_eq!(s_after.turns.len(), 1);
        assert!(s_after.turns[0].content.contains("J-SPACE HANDOVER"));
        assert_eq!(s_after.shared_state.get("active_model").unwrap(), "hcs-coder");
    }
}

