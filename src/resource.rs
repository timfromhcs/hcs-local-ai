use sysinfo::System;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemResourceStats {
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub available_memory_mb: u64,
    pub memory_usage_percent: f32,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub max_heavy_active: usize,
    pub active_heavy_count: usize,
    pub system_reserve_mb: u64,
    pub emergency_reserve_mb: u64,
}

pub struct ResourceManager {
    sys: Mutex<System>,
    heavy_semaphore: Arc<Semaphore>,
    max_heavy_active: usize,
    system_reserve_mb: u64,
    emergency_reserve_mb: u64,
    active_heavy_count: Arc<Mutex<usize>>,
}

impl ResourceManager {
    pub fn new(max_heavy_active: usize, system_reserve_mb: u64, emergency_reserve_mb: u64) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: Mutex::new(sys),
            heavy_semaphore: Arc::new(Semaphore::new(max_heavy_active)),
            max_heavy_active,
            system_reserve_mb,
            emergency_reserve_mb,
            active_heavy_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn is_heavy_model(model_id: &str) -> bool {
        matches!(model_id, "hcs-coder" | "hcs-vlm" | "hcs-image")
    }

    /// Accurate estimate of model RAM/VRAM footprint in MB (including 64k Q4_0 KV cache and compute buffers).
    pub fn estimate_model_memory_mb(model_id: &str) -> u64 {
        match model_id {
            "hcs-subagent" => 650,    // 248 MB weights + 64k Q4_0 KV cache + buffers
            "hcs-general"  => 2300,   // 1.07 GB weights + 64k Q4_0 KV cache + buffers
            "hcs-judge"    => 3900,   // 2.7 GB weights + 64k Q4_0 KV cache + buffers
            "hcs-coder"    => 11200,  // 7.2 GB weights + 64k Q4_0 KV cache + buffers (fits in 12.5GB free UMA)
            "hcs-vlm"      => 9600,   // 7.7 GB weights + 64k Q4_0 KV cache + mmproj + buffers
            "hcs-image"    => 4500,   // FLUX.2 Klein + Qwen3 encoder + VAE + buffers
            _              => 2000,
        }
    }

    /// Optimal, crash-safe Vulkan GPU layer offloading for AMD iGPU (4GB AdapterRAM / 20GB shared UMA).
    pub fn get_safe_gpu_layers(model_id: &str) -> u32 {
        match model_id {
            "hcs-subagent" => 99, // Tiny (248MB) - fits 100% in VRAM
            "hcs-general"  => 99, // 1.07GB - fits safely in VRAM
            "hcs-judge"    => 32, // Fits in ~2.5GB VRAM
            "hcs-coder"    => 32, // Accelerates hot layers in VRAM, avoids AMD driver GTT thrashing
            "hcs-vlm"      => 28, // Safely within VRAM limits alongside mmproj
            _              => 32,
        }
    }

    pub fn get_stats(&self) -> SystemResourceStats {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        let total_b = sys.total_memory();
        let avail_b = sys.available_memory();
        let used_b = sys.used_memory();

        let total_mb = total_b / (1024 * 1024);
        let avail_mb = avail_b / (1024 * 1024);
        let used_mb = used_b / (1024 * 1024);
        let mem_pct = if total_b > 0 { (used_b as f32 / total_b as f32) * 100.0 } else { 0.0 };

        let cpu_pct = sys.global_cpu_usage();
        let active_heavy = *self.active_heavy_count.lock().unwrap();

        SystemResourceStats {
            total_memory_mb: total_mb,
            used_memory_mb: used_mb,
            available_memory_mb: avail_mb,
            memory_usage_percent: mem_pct,
            cpu_count: sys.cpus().len(),
            cpu_usage_percent: cpu_pct,
            max_heavy_active: self.max_heavy_active,
            active_heavy_count: active_heavy,
            system_reserve_mb: self.system_reserve_mb,
            emergency_reserve_mb: self.emergency_reserve_mb,
        }
    }

    pub fn get_available_memory_mb(&self) -> u64 {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_memory();
        sys.available_memory() / (1024 * 1024)
    }

    /// Check if loading this model is safe under current system memory pressure.
    pub fn check_memory_available(&self, estimated_need_mb: u64) -> bool {
        let avail_mb = self.get_available_memory_mb();
        let required = estimated_need_mb + self.emergency_reserve_mb;
        avail_mb >= required
    }

    /// Determine which active models MUST be evicted before loading `target_model`.
    /// On a 20GB shared UMA system, we strictly enforce:
    /// - Max 1 heavy model (coder, vlm, image) at any time.
    /// - Heavy models evict all non-subagent models.
    /// - If memory is tight, evict all other models including subagent.
    pub fn determine_evictions_for_load(&self, target_model: &str, active_models: &[String]) -> Vec<String> {
        let mut evict = Vec::new();
        let is_target_heavy = Self::is_heavy_model(target_model);
        let needed_mb = Self::estimate_model_memory_mb(target_model);
        let avail_mb = self.get_available_memory_mb();

        for active in active_models {
            if active == target_model {
                continue;
            }

            // 1. If loading a heavy model, evict all other heavy and medium models
            if is_target_heavy {
                if active != "hcs-subagent" {
                    evict.push(active.clone());
                }
            } else if Self::is_heavy_model(active) {
                // 2. If target is non-heavy, but an active model IS heavy, evict the heavy model
                evict.push(active.clone());
            } else if active != "hcs-subagent" && (active == "hcs-judge" || active == "hcs-general") {
                // 3. For medium models (judge / general), avoid running both at once if memory is tight
                if avail_mb < needed_mb + self.system_reserve_mb {
                    evict.push(active.clone());
                }
            }
        }

        // 4. Extreme memory pressure check: If after planned evictions, memory is still
        // dangerously low, also evict hcs-subagent
        if active_models.contains(&"hcs-subagent".to_string()) && target_model != "hcs-subagent" {
            let estimated_after_evict = avail_mb + evict.iter()
                .map(|m| Self::estimate_model_memory_mb(m))
                .sum::<u64>();
            if estimated_after_evict < needed_mb + self.system_reserve_mb {
                evict.push("hcs-subagent".to_string());
            }
        }

        evict
    }

    pub async fn acquire_heavy_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        let permit = self.heavy_semaphore.clone().acquire_owned().await.unwrap();
        let mut count = self.active_heavy_count.lock().unwrap();
        *count += 1;
        permit
    }

    pub fn release_heavy_permit(&self) {
        let mut count = self.active_heavy_count.lock().unwrap();
        if *count > 0 {
            *count -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_manager_heavy_concurrency() {
        let rm = ResourceManager::new(1, 2048, 1024);
        assert!(ResourceManager::is_heavy_model("hcs-coder"));
        assert!(ResourceManager::is_heavy_model("hcs-vlm"));
        assert!(ResourceManager::is_heavy_model("hcs-image"));
        assert!(!ResourceManager::is_heavy_model("hcs-subagent"));
        assert!(!ResourceManager::is_heavy_model("hcs-judge"));

        let permit = rm.acquire_heavy_permit().await;
        let stats = rm.get_stats();
        assert_eq!(stats.active_heavy_count, 1);

        drop(permit);
        rm.release_heavy_permit();
        let stats2 = rm.get_stats();
        assert_eq!(stats2.active_heavy_count, 0);

        // Check memory estimation and safe layer queries
        assert_eq!(ResourceManager::get_safe_gpu_layers("hcs-coder"), 32);
        assert_eq!(ResourceManager::get_safe_gpu_layers("hcs-subagent"), 99);
        assert!(ResourceManager::estimate_model_memory_mb("hcs-coder") >= 7000);

        // Check eviction logic: loading hcs-coder should evict hcs-judge
        let active = vec!["hcs-judge".to_string(), "hcs-subagent".to_string()];
        let evictions = rm.determine_evictions_for_load("hcs-coder", &active);
        assert!(evictions.contains(&"hcs-judge".to_string()));
    }
}

