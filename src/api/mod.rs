pub mod auth;
pub mod openai;
pub mod anthropic;
pub mod hcs;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, Method};
use axum::routing::{delete, get, post};
use axum::Router;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::{info, warn};

use crate::agent::AgentRuntime;
use crate::backend::prism::PrismWorker;
use crate::backend::sd::StableDiffusionWorker;
use crate::config::Config;
use crate::db::Database;
use crate::models::{ModelLifecycleState, ModelRegistry};
use crate::resource::ResourceManager;
use crate::telemetry::TelemetryTracker;
use crate::watchdog::Watchdog;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub registry: ModelRegistry,
    pub resource: Arc<ResourceManager>,
    pub telemetry: TelemetryTracker,
    pub watchdog: Arc<Watchdog>,
    pub agent: Arc<AgentRuntime>,
    pub jspace: Arc<crate::j_space::JSpaceManager>,
    pub brain: Arc<crate::brain::PersistentBrain>,
    pub active_workers: Arc<Mutex<HashMap<String, PrismWorker>>>,
    pub sd_worker: Arc<StableDiffusionWorker>,
    pub event_tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn broadcast_event<T: serde::Serialize>(&self, event_type: &str, data: &T) {
        let msg = serde_json::json!({
            "event": event_type,
            "data": data,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }).to_string();
        let _ = self.event_tx.send(msg);
    }

    pub async fn ensure_worker(&self, model_id: &str) -> anyhow::Result<u16> {
        let mut workers = self.active_workers.lock().await;

        if let Some(worker) = workers.get_mut(model_id) {
            if worker.is_alive() {
                self.registry.update_state(model_id, ModelLifecycleState::Active);
                return Ok(worker.port);
            } else {
                warn!("Worker for {} was dead, removing from active pool...", model_id);
                self.watchdog.handle_worker_crash(model_id);
                workers.remove(model_id);
            }
        }

        let model_info = self.registry.get_model(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if model_info.state == ModelLifecycleState::Quarantined {
            anyhow::bail!("Model {} is QUARANTINED due to consecutive crashes", model_id);
        }

        let is_heavy = ResourceManager::is_heavy_model(model_id);

        // Smart Offloading Policy: Determine which active models must be evicted
        let active_ids: Vec<String> = workers.keys().cloned().collect();
        let to_evict = self.resource.determine_evictions_for_load(model_id, &active_ids);
        for id in to_evict {
            info!("Smart Offload: Evicting model '{}' to free memory before loading '{}'...", id, model_id);
            if let Some(mut old_worker) = workers.remove(&id) {
                old_worker.stop().await;
                if ResourceManager::is_heavy_model(&id) {
                    self.resource.release_heavy_permit();
                }
                self.registry.update_state(&id, ModelLifecycleState::Cold);
            }
        }

        // Memory Safety Gate: Ensure sufficient RAM/VRAM is available to prevent system lockup
        let needed_mb = ResourceManager::estimate_model_memory_mb(model_id);
        if !self.resource.check_memory_available(needed_mb) {
            let avail_mb = self.resource.get_available_memory_mb();
            warn!("Memory safety threshold check warning for {}: Available {} MB, Need {} MB", model_id, avail_mb, needed_mb);
            let remaining: Vec<String> = workers.keys().filter(|&k| k != model_id).cloned().collect();
            for id in remaining {
                info!("Emergency Offload: Evicting '{}'...", id);
                if let Some(mut old_worker) = workers.remove(&id) {
                    old_worker.stop().await;
                    if ResourceManager::is_heavy_model(&id) {
                        self.resource.release_heavy_permit();
                    }
                    self.registry.update_state(&id, ModelLifecycleState::Cold);
                }
            }
            if !self.resource.check_memory_available(needed_mb) {
                let avail_mb = self.resource.get_available_memory_mb();
                anyhow::bail!(
                    "Cannot safely load {}: Insufficient memory ({:.1} GB available, need {:.1} GB). Refusing load to protect system stability.",
                    model_id, (avail_mb as f64) / 1024.0, (needed_mb as f64) / 1024.0
                );
            }
        }

        self.registry.update_state(model_id, ModelLifecycleState::Loading);

        // Find available port between 8790 and 8850
        let mut port = 8790;
        let used_ports: Vec<u16> = workers.values().map(|w| w.port).collect();
        while used_ports.contains(&port) {
            port += 1;
        }

        let prism_bin = self.config.storage.runtime_dir.join("windows-x64/prism/llama-server.exe");
        let mmproj_path = model_info.manifest.mmproj.as_ref().map(|f| model_info.directory.join(f));
        let ctx_len = std::cmp::min(model_info.manifest.context.unwrap_or(4096), if is_heavy { 4096 } else { 8192 });
        let flash_attn = &self.config.resources.flash_attention;
        let gpu_layers = ResourceManager::get_safe_gpu_layers(model_id);
        let threads = self.config.resources.threads;
        let batch_size = std::cmp::min(self.config.resources.batch_size, 512);
        let ubatch_size = std::cmp::min(self.config.resources.ubatch_size, 128);
        let cache_type_k = &self.config.resources.cache_type_k;
        let cache_type_v = &self.config.resources.cache_type_v;

        let worker = PrismWorker::start(
            model_id,
            &prism_bin,
            &model_info.file_path,
            mmproj_path.as_deref(),
            port,
            ctx_len,
            gpu_layers,
            threads,
            batch_size,
            ubatch_size,
            flash_attn,
            cache_type_k,
            cache_type_v,
        ).await?;

        if is_heavy {
            let _ = self.resource.acquire_heavy_permit().await;
        }

        self.registry.set_worker(model_id, port, 0);
        workers.insert(model_id.to_string(), worker);

        self.broadcast_event("model_loaded", &serde_json::json!({
            "model_id": model_id,
            "port": port,
            "state": "READY"
        }));

        Ok(port)
    }
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT, header::HeaderName::from_static("x-api-key")]);

    // Serve generated artifacts folder
    let artifacts_dir = state.config.storage.artifacts_dir.clone();
    let dashboard_dir = PathBuf::from("dashboard");

    Router::new()
        // OpenAI compatibility routes
        .route("/v1/models", get(openai::list_models))
        .route("/v1/chat/completions", post(openai::chat_completions))
        .route("/v1/completions", post(openai::completions))
        .route("/v1/responses", post(openai::responses))
        .route("/v1/images/generations", post(openai::images_generations))
        .route("/v1/images/edits", post(openai::images_edits))
        .route("/v1/files", get(openai::list_files).post(openai::upload_file))
        .route("/v1/files/{id}", get(openai::get_file).delete(openai::delete_file))
        .route("/v1/files/{id}/content", get(openai::get_file_content))
        .route("/v1/batches", get(openai::create_batch).post(openai::create_batch))
        .route("/v1/batches/{id}", get(openai::get_batch))

        // Anthropic compatibility routes
        .route("/v1/messages", post(anthropic::create_message))
        .route("/v1/messages/count_tokens", post(anthropic::count_tokens))

        // HCS native management routes
        .route("/hcs/v1/system", get(hcs::system_info))
        .route("/hcs/v1/doctor", get(hcs::doctor_report))
        .route("/hcs/v1/models", get(hcs::list_hcs_models))
        .route("/hcs/v1/models/{id}/load", post(hcs::load_model))
        .route("/hcs/v1/models/{id}/unload", post(hcs::unload_model))
        .route("/hcs/v1/decision", post(hcs::decision_contract))
        .route("/hcs/v1/memory", get(hcs::get_memory).post(hcs::store_memory))
        .route("/hcs/v1/memory/{id}", delete(hcs::delete_memory))
        .route("/hcs/v1/agent", get(hcs::system_info))
        .route("/hcs/v1/agent/run", post(hcs::run_agent))
        .route("/hcs/v1/telemetry", get(hcs::get_telemetry))
        .route("/hcs/v1/requests", get(hcs::get_recent_requests))
        .route("/hcs/v1/keys", get(hcs::list_keys).post(hcs::create_key))
        .route("/hcs/v1/keys/{id}/revoke", post(hcs::revoke_key))
        .route("/hcs/v1/events", get(hcs::live_events))

        // HCS v2 routes (J-Space, Jev Delegation, Brain Auto-Learning, Hardware)
        .route("/hcs/v2/jspace/sessions", get(hcs::list_jspace_sessions).post(hcs::create_jspace_session))
        .route("/hcs/v2/jspace/sessions/{id}", get(hcs::get_jspace_session).delete(hcs::delete_jspace_session))
        .route("/hcs/v2/jspace/sessions/{id}/state", post(hcs::set_jspace_state))
        .route("/hcs/v2/jspace/sessions/{id}/turns", post(hcs::append_jspace_turn))
        .route("/hcs/v2/jev/delegate", post(hcs::jev_delegate))
        .route("/hcs/v2/jev/rate_plan", post(hcs::jev_rate_plan))
        .route("/hcs/v2/brain/learn", post(hcs::brain_learn))
        .route("/hcs/v2/brain/recall", get(hcs::brain_recall))
        .route("/hcs/v2/brain/insights", get(hcs::brain_insights))
        .route("/hcs/v2/hardware/profile", get(hcs::hardware_profile))
        .route("/hcs/v2/compact", post(hcs::compact_context))
        .route("/hcs/v2/jspace/sessions/{id}/handover", post(hcs::jspace_handover))
        .route("/hcs/v2/jspace/sessions/{id}/variables", get(hcs::get_jspace_variables).post(hcs::set_jspace_variable))


        // Static routes for artifacts and dashboard UI
        .nest_service("/artifacts", ServeDir::new(artifacts_dir))
        .fallback_service(ServeDir::new(dashboard_dir))
        .layer(cors)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100MB body limit
        .with_state(state)
}
