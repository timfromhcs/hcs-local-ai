use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub resources: ResourceConfig,
    #[serde(default)]
    pub jspace: JSpaceConfig,
    pub watchdog: WatchdogConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub models_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub data_dir: PathBuf,
    pub artifacts_dir: PathBuf,
}

fn default_q8() -> String { "q8_0".to_string() }
fn default_threads() -> usize { 8 }
fn default_batch_size() -> usize { 1024 }
fn default_ubatch_size() -> usize { 256 }
fn default_max_sessions() -> usize { 64 }
fn default_ttl_secs() -> u64 { 86400 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub max_heavy_active: usize,
    pub system_reserve_mb: u64,
    pub emergency_reserve_mb: u64,
    pub flash_attention: String, // "auto", "on", "off"
    #[serde(default = "default_q8")]
    pub cache_type_k: String,
    #[serde(default = "default_q8")]
    pub cache_type_v: String,
    #[serde(default = "default_threads")]
    pub threads: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_ubatch_size")]
    pub ubatch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSpaceConfig {
    #[serde(default = "default_max_sessions")]
    pub max_active_sessions: usize,
    #[serde(default = "default_ttl_secs")]
    pub session_ttl_secs: u64,
}

impl Default for JSpaceConfig {
    fn default() -> Self {
        Self {
            max_active_sessions: 64,
            session_ttl_secs: 86400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    pub enabled: bool,
    pub check_interval_secs: u64,
    pub max_crashes_before_quarantine: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub initial_admin_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8787,
                cors_allowed_origins: vec!["*".to_string()],
            },
            storage: StorageConfig {
                models_dir: PathBuf::from("models"),
                runtime_dir: PathBuf::from("runtime"),
                data_dir: PathBuf::from("data"),
                artifacts_dir: PathBuf::from("artifacts"),
            },
            resources: ResourceConfig {
                max_heavy_active: 1,
                system_reserve_mb: 3072,
                emergency_reserve_mb: 1024,
                flash_attention: "auto".to_string(),
                cache_type_k: "q8_0".to_string(),
                cache_type_v: "q8_0".to_string(),
                threads: 8,
                batch_size: 1024,
                ubatch_size: 256,
            },
            jspace: JSpaceConfig::default(),
            watchdog: WatchdogConfig {
                enabled: true,
                check_interval_secs: 5,
                max_crashes_before_quarantine: 3,
            },
            auth: AuthConfig {
                enabled: false,
                initial_admin_key: None,
            },
        }
    }
}

impl Config {
    pub fn load_or_default(path: Option<&Path>) -> Self {
        if let Some(p) = path {
            if p.exists() {
                if let Ok(content) = std::fs::read_to_string(p) {
                    if let Ok(cfg) = serde_yaml::from_str(&content) {
                        return cfg;
                    }
                }
            }
        }
        let default_cfg_file = Path::new("hcs.yaml");
        if default_cfg_file.exists() {
            if let Ok(content) = std::fs::read_to_string(default_cfg_file) {
                if let Ok(cfg) = serde_yaml::from_str(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = serde_yaml::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_and_save() {
        let cfg = Config::default();
        assert_eq!(cfg.server.port, 8787);
        assert_eq!(cfg.resources.max_heavy_active, 1);

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test_config.yaml");
        cfg.save(&path).unwrap();

        let loaded = Config::load_or_default(Some(&path));
        assert_eq!(loaded.server.port, 8787);
    }
}

