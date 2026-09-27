use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub id: String,
    pub source_repository: String,
    pub source_revision: String,
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub backend: String,
    pub platform: String,
    pub capabilities: Vec<String>,
    pub context: Option<usize>,
    pub quantization: String,
    pub mtp: bool,
    pub mmproj: Option<String>,
    pub dependencies: Vec<String>,
    pub license: String,
    pub license_checked_at: String,
    pub verification_status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelLifecycleState {
    Cold,
    Validating,
    Loading,
    Warming,
    Ready,
    Active,
    Idle,
    Sleeping,
    Evicting,
    Failed,
    Quarantined,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRuntimeInfo {
    pub manifest: ModelManifest,
    pub directory: PathBuf,
    pub file_path: PathBuf,
    pub state: ModelLifecycleState,
    pub active_requests: usize,
    pub loaded_at: Option<String>,
    pub last_used_at: Option<String>,
    pub worker_port: Option<u16>,
    pub worker_pid: Option<u32>,
    pub crash_count: u32,
    pub kv_cache_profile: String,
}

#[derive(Clone)]
pub struct ModelRegistry {
    models: Arc<RwLock<HashMap<String, ModelRuntimeInfo>>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn scan_and_load<P: AsRef<Path>>(&self, models_dir: P) -> anyhow::Result<()> {
        let dir = models_dir.as_ref();
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }

        let mut map = self.models.write().unwrap();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.yaml");
                if manifest_path.exists() {
                    let content = std::fs::read_to_string(&manifest_path)?;
                    if let Ok(manifest) = serde_yaml::from_str::<ModelManifest>(&content) {
                        let file_path = path.join(&manifest.filename);
                        let file_exists = file_path.exists();
                        let state = if file_exists {
                            ModelLifecycleState::Cold
                        } else {
                            ModelLifecycleState::Failed
                        };

                        map.insert(
                            manifest.id.clone(),
                            ModelRuntimeInfo {
                                manifest,
                                directory: path.clone(),
                                file_path,
                                state,
                                active_requests: 0,
                                loaded_at: None,
                                last_used_at: None,
                                worker_port: None,
                                worker_pid: None,
                                crash_count: 0,
                                kv_cache_profile: "balanced".to_string(),
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_model(&self, id: &str) -> Option<ModelRuntimeInfo> {
        let map = self.models.read().unwrap();
        map.get(id).cloned()
    }

    pub fn list_models(&self) -> Vec<ModelRuntimeInfo> {
        let map = self.models.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn update_state(&self, id: &str, state: ModelLifecycleState) {
        let mut map = self.models.write().unwrap();
        if let Some(m) = map.get_mut(id) {
            m.state = state;
            if state == ModelLifecycleState::Ready || state == ModelLifecycleState::Active {
                if m.loaded_at.is_none() {
                    m.loaded_at = Some(chrono::Utc::now().to_rfc3339());
                }
                m.last_used_at = Some(chrono::Utc::now().to_rfc3339());
            } else if state == ModelLifecycleState::Cold {
                m.loaded_at = None;
                m.worker_port = None;
                m.worker_pid = None;
            }
        }
    }

    pub fn set_worker(&self, id: &str, port: u16, pid: u32) {
        let mut map = self.models.write().unwrap();
        if let Some(m) = map.get_mut(id) {
            m.worker_port = Some(port);
            m.worker_pid = Some(pid);
            m.state = ModelLifecycleState::Ready;
            m.loaded_at = Some(chrono::Utc::now().to_rfc3339());
            m.last_used_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    pub fn record_crash(&self, id: &str) -> u32 {
        let mut map = self.models.write().unwrap();
        if let Some(m) = map.get_mut(id) {
            m.crash_count += 1;
            m.worker_port = None;
            m.worker_pid = None;
            m.crash_count
        } else {
            0
        }
    }

    pub fn resolve_alias(&self, requested: &str) -> String {
        match requested {
            "auto" => "hcs-general".to_string(),
            "subagent" | "hcs-subagent" => "hcs-subagent".to_string(),
            "general" | "hcs-general" => "hcs-general".to_string(),
            "coder" | "hcs-coder" => "hcs-coder".to_string(),
            "judge" | "hcs-judge" => "hcs-judge".to_string(),
            "vlm" | "vision" | "hcs-vlm" => "hcs-vlm".to_string(),
            "image" | "flux" | "flux2" | "hcs-image" => "hcs-image".to_string(),
            other => {
                // If it matches an ID directly
                let map = self.models.read().unwrap();
                if map.contains_key(other) {
                    other.to_string()
                } else if other.contains("coder") || other.contains("code") {
                    "hcs-coder".to_string()
                } else if other.contains("vision") || other.contains("vlm") || other.contains("vl") {
                    "hcs-vlm".to_string()
                } else if other.contains("flux") || other.contains("image") {
                    "hcs-image".to_string()
                } else if other.contains("judge") || other.contains("openjev") {
                    "hcs-judge".to_string()
                } else {
                    "hcs-general".to_string()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_alias_resolution() {
        let registry = ModelRegistry::new();
        assert_eq!(registry.resolve_alias("auto"), "hcs-general");
        assert_eq!(registry.resolve_alias("coder"), "hcs-coder");
        assert_eq!(registry.resolve_alias("hcs-coder"), "hcs-coder");
        assert_eq!(registry.resolve_alias("subagent"), "hcs-subagent");
        assert_eq!(registry.resolve_alias("vlm"), "hcs-vlm");
        assert_eq!(registry.resolve_alias("vision"), "hcs-vlm");
        assert_eq!(registry.resolve_alias("image"), "hcs-image");
        assert_eq!(registry.resolve_alias("flux"), "hcs-image");
        assert_eq!(registry.resolve_alias("judge"), "hcs-judge");
        assert_eq!(registry.resolve_alias("unknown-thing"), "hcs-general");
    }

    #[test]
    fn test_model_lifecycle_transitions() {
        let registry = ModelRegistry::new();
        // Scanning local models directory
        registry.scan_and_load("models").unwrap();
        let models = registry.list_models();
        assert!(!models.is_empty());

        let subagent = registry.get_model("hcs-subagent");
        assert!(subagent.is_some());

        registry.update_state("hcs-subagent", ModelLifecycleState::Loading);
        assert_eq!(registry.get_model("hcs-subagent").unwrap().state, ModelLifecycleState::Loading);

        registry.set_worker("hcs-subagent", 8800, 1234);
        let ready = registry.get_model("hcs-subagent").unwrap();
        assert_eq!(ready.state, ModelLifecycleState::Ready);
        assert_eq!(ready.worker_port, Some(8800));

        let crash_count = registry.record_crash("hcs-subagent");
        assert_eq!(crash_count, 1);
        assert_eq!(registry.get_model("hcs-subagent").unwrap().worker_port, None);
    }
}

