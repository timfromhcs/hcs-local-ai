use crate::config::Config;
use crate::models::ModelRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheckItem {
    pub name: String,
    pub status: String, // "PASS", "WARN", "FAIL"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp: String,
    pub overall_status: String,
    pub checks: Vec<DoctorCheckItem>,
}

pub struct Doctor;

impl Doctor {
    pub fn run_diagnostics(config: &Config, registry: &ModelRegistry) -> DoctorReport {
        let mut checks = Vec::new();
        let mut overall_ok = true;

        // 1. Operating System
        checks.push(DoctorCheckItem {
            name: "Operating System".to_string(),
            status: "PASS".to_string(),
            message: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        });

        // 2. Memory
        let mut sys = sysinfo::System::new_all();
        sys.refresh_memory();
        let total_gb = (sys.total_memory() as f64) / (1024.0 * 1024.0 * 1024.0);
        let avail_gb = (sys.available_memory() as f64) / (1024.0 * 1024.0 * 1024.0);
        checks.push(DoctorCheckItem {
            name: "Unified System Memory".to_string(),
            status: if total_gb >= 16.0 { "PASS".to_string() } else { "WARN".to_string() },
            message: format!("{:.1} GB total, {:.1} GB available", total_gb, avail_gb),
        });

        // 3. Runtimes
        let prism_bin = config.storage.runtime_dir.join("windows-x64/prism/llama-server.exe");
        let sd_bin = config.storage.runtime_dir.join("windows-x64/sd-cpp/sd-cli.exe");

        if prism_bin.exists() {
            checks.push(DoctorCheckItem {
                name: "Prism Vulkan Runtime".to_string(),
                status: "PASS".to_string(),
                message: format!("Binary present at {:?}", prism_bin),
            });
        } else {
            overall_ok = false;
            checks.push(DoctorCheckItem {
                name: "Prism Vulkan Runtime".to_string(),
                status: "FAIL".to_string(),
                message: format!("Missing binary at {:?}", prism_bin),
            });
        }

        if sd_bin.exists() {
            checks.push(DoctorCheckItem {
                name: "stable-diffusion.cpp Vulkan Runtime".to_string(),
                status: "PASS".to_string(),
                message: format!("Binary present at {:?}", sd_bin),
            });
        } else {
            overall_ok = false;
            checks.push(DoctorCheckItem {
                name: "stable-diffusion.cpp Vulkan Runtime".to_string(),
                status: "FAIL".to_string(),
                message: format!("Missing binary at {:?}", sd_bin),
            });
        }

        // 4. Models
        let required_models = ["hcs-subagent", "hcs-general", "hcs-coder", "hcs-judge", "hcs-vlm", "hcs-image"];
        for m in required_models {
            if let Some(info) = registry.get_model(m) {
                if info.file_path.exists() {
                    checks.push(DoctorCheckItem {
                        name: format!("Model: {}", m),
                        status: "PASS".to_string(),
                        message: format!("Verified on disk ({:.2} GB, file: {})", (info.manifest.size_bytes as f64) / (1024.0 * 1024.0 * 1024.0), info.manifest.filename),
                    });
                } else {
                    overall_ok = false;
                    checks.push(DoctorCheckItem {
                        name: format!("Model: {}", m),
                        status: "FAIL".to_string(),
                        message: format!("File missing at {:?}", info.file_path),
                    });
                }
            } else {
                overall_ok = false;
                checks.push(DoctorCheckItem {
                    name: format!("Model: {}", m),
                    status: "FAIL".to_string(),
                    message: "Model not found in registry manifests".to_string(),
                });
            }
        }

        // 5. Database check
        let db_path = config.storage.data_dir.join("hcs.db");
        checks.push(DoctorCheckItem {
            name: "Persistent SQLite Storage".to_string(),
            status: "PASS".to_string(),
            message: format!("Database location configured at {:?}", db_path),
        });

        DoctorReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            overall_status: if overall_ok { "HEALTHY".to_string() } else { "DEGRADED".to_string() },
            checks,
        }
    }
}
