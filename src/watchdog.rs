use crate::models::{ModelLifecycleState, ModelRegistry};
use tracing::{error, info, warn};

pub struct Watchdog {
    registry: ModelRegistry,
    max_crashes: u32,
}

impl Watchdog {
    pub fn new(registry: ModelRegistry, max_crashes: u32) -> Self {
        Self {
            registry,
            max_crashes,
        }
    }

    pub fn handle_worker_crash(&self, model_id: &str) {
        let count = self.registry.record_crash(model_id);
        warn!("Worker crash recorded for model '{}', total crashes: {}", model_id, count);

        if count >= self.max_crashes {
            error!("Model '{}' exceeded max crash limit ({}), putting into QUARANTINED state", model_id, self.max_crashes);
            self.registry.update_state(model_id, ModelLifecycleState::Quarantined);
        } else {
            info!("Model '{}' will be restarted on next incoming request (state reset to COLD)", model_id);
            self.registry.update_state(model_id, ModelLifecycleState::Cold);
        }
    }

    /// Background task that continuously monitors system memory.
    /// If available RAM drops below emergency reserve, evicts largest worker to prevent PC freeze.
    pub fn start_memory_guard(
        resource: std::sync::Arc<crate::resource::ResourceManager>,
        active_workers: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::backend::prism::PrismWorker>>>,
        registry: ModelRegistry,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
            loop {
                interval.tick().await;
                let avail_mb = resource.get_available_memory_mb();
                let stats = resource.get_stats();
                if avail_mb < stats.emergency_reserve_mb {
                    warn!("WATCHDOG OOM ALERT: Free physical memory ({} MB) dropped below emergency reserve ({} MB)!", avail_mb, stats.emergency_reserve_mb);
                    let mut workers = active_workers.lock().await;
                    let to_evict: Option<String> = workers.keys()
                        .max_by_key(|id| crate::resource::ResourceManager::estimate_model_memory_mb(id))
                        .cloned();
                    if let Some(id) = to_evict {
                        error!("WATCHDOG: Forcefully evicting worker '{}' to safeguard OS stability", id);
                        if let Some(mut worker) = workers.remove(&id) {
                            worker.stop().await;
                            if crate::resource::ResourceManager::is_heavy_model(&id) {
                                resource.release_heavy_permit();
                            }
                            registry.update_state(&id, ModelLifecycleState::Cold);
                        }
                    }
                }
            }
        });
    }
}
