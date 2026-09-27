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

    pub fn check_memory_available(&self, estimated_need_mb: u64) -> bool {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_memory();
        let avail_mb = sys.available_memory() / (1024 * 1024);
        let required = estimated_need_mb + self.system_reserve_mb;
        avail_mb >= required
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
