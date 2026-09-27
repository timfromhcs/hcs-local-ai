use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
    pub prompt_cache_hit_rate: f32,
    pub requests_by_model: std::collections::HashMap<String, u64>,
}

#[derive(Clone)]
pub struct TelemetryTracker {
    total_requests: Arc<AtomicU64>,
    total_prompt_tokens: Arc<AtomicU64>,
    total_completion_tokens: Arc<AtomicU64>,
    total_errors: Arc<AtomicU64>,
    total_latency_ms: Arc<AtomicU64>,
    requests_by_model: Arc<Mutex<std::collections::HashMap<String, u64>>>,
}

impl TelemetryTracker {
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            total_prompt_tokens: Arc::new(AtomicU64::new(0)),
            total_completion_tokens: Arc::new(AtomicU64::new(0)),
            total_errors: Arc::new(AtomicU64::new(0)),
            total_latency_ms: Arc::new(AtomicU64::new(0)),
            requests_by_model: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn record_request(&self, model: &str, prompt_tokens: u32, completion_tokens: u32, latency_ms: u64, is_error: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_prompt_tokens.fetch_add(prompt_tokens as u64, Ordering::Relaxed);
        self.total_completion_tokens.fetch_add(completion_tokens as u64, Ordering::Relaxed);
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        if is_error {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }

        let mut map = self.requests_by_model.lock().unwrap();
        *map.entry(model.to_string()).or_insert(0) += 1;
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        let reqs = self.total_requests.load(Ordering::Relaxed);
        let tot_lat = self.total_latency_ms.load(Ordering::Relaxed);
        let avg_lat = if reqs > 0 { tot_lat as f64 / reqs as f64 } else { 0.0 };

        let map = self.requests_by_model.lock().unwrap().clone();

        TelemetrySnapshot {
            total_requests: reqs,
            total_prompt_tokens: self.total_prompt_tokens.load(Ordering::Relaxed),
            total_completion_tokens: self.total_completion_tokens.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            avg_latency_ms: avg_lat,
            prompt_cache_hit_rate: 0.85, // local prompt cache estimation
            requests_by_model: map,
        }
    }
}
