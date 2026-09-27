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
}
