mod agent;
mod api;
mod backend;
pub mod brain;
mod config;
mod db;
mod doctor;
pub mod j_space;
mod models;
mod openjev;
pub mod openjev_pipeline;
mod resource;
mod telemetry;
mod watchdog;

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use agent::{AgentRuntime, ToolExecutor};
use backend::sd::StableDiffusionWorker;
use config::Config;
use db::Database;
use doctor::Doctor;
use models::ModelRegistry;
use resource::ResourceManager;
use telemetry::TelemetryTracker;
use watchdog::Watchdog;

#[derive(Parser)]
#[command(name = "hcs-daemon")]
#[command(version)]
#[command(about = "HCS Local AI unified daemon - OpenAI & Anthropic compatible API, Prism & SD.cpp Vulkan runtime orchestration")]
struct Cli {
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HCS daemon server (default)
    Run {
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Run diagnostic checks on system hardware, runtimes, and models
    Doctor,
    /// Model management commands
    Models {
        #[command(subcommand)]
        sub: ModelCommands,
    },
    /// API key management
    Keys {
        #[command(subcommand)]
        sub: KeyCommands,
    },
}

#[derive(Subcommand)]
enum ModelCommands {
    /// List all discovered models and manifests
    List,
}

#[derive(Subcommand)]
enum KeyCommands {
    /// Create a new API key
    Create {
        #[arg(short, long)]
        name: String,
    },
    /// List all existing API keys
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();
    let config = Config::load_or_default(cli.config.as_deref());

    // Ensure required storage directories exist
    tokio::fs::create_dir_all(&config.storage.data_dir).await?;
    tokio::fs::create_dir_all(&config.storage.artifacts_dir).await?;

    let db_path = config.storage.data_dir.join("hcs.db");
    let db = Database::init(&db_path)?;

    let registry = ModelRegistry::new();
    registry.scan_and_load(&config.storage.models_dir)?;

    match cli.command.unwrap_or(Commands::Run { port: None }) {
        Commands::Doctor => {
            let report = Doctor::run_diagnostics(&config, &registry);
            println!("\n=== HCS Local AI Doctor Diagnostics ===");
            println!("Overall Status: {}\n", report.overall_status);
            for check in report.checks {
                println!("[{}] {}: {}", check.status, check.name, check.message);
            }
            println!("========================================\n");
            Ok(())
        }
        Commands::Models { sub } => match sub {
            ModelCommands::List => {
                println!("\n=== HCS Models Registry ===");
                for m in registry.list_models() {
                    println!("• {} ({})", m.manifest.id, m.manifest.quantization);
                    println!("  File: {:?}", m.file_path);
                    println!("  Backend: {}", m.manifest.backend);
                    println!("  Context: {:?}", m.manifest.context);
                    println!("  Capabilities: {:?}", m.manifest.capabilities);
                    println!();
                }
                Ok(())
            }
        },
        Commands::Keys { sub } => match sub {
            KeyCommands::Create { name } => {
                let raw_key = format!("hcs-key-{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
                let record = db.insert_api_key(&name, &raw_key, &["*".to_string()])?;
                println!("Created API Key: {}", record.name);
                println!("Secret Key: {}", raw_key);
                println!("WARNING: Store this key safely. It cannot be retrieved later.");
                Ok(())
            }
            KeyCommands::List => {
                println!("\n=== HCS API Keys ===");
                for k in db.list_api_keys()? {
                    println!("• ID: {} | Name: {} | Revoked: {}", k.id, k.name, k.is_revoked);
                }
                Ok(())
            }
        },
        Commands::Run { port } => {
            let listen_port = port.unwrap_or(config.server.port);

            let resource = Arc::new(ResourceManager::new(
                config.resources.max_heavy_active,
                config.resources.system_reserve_mb,
                config.resources.emergency_reserve_mb,
            ));

            let telemetry = TelemetryTracker::new();
            let watchdog = Arc::new(Watchdog::new(registry.clone(), config.watchdog.max_crashes_before_quarantine));

            let tool_executor = Arc::new(ToolExecutor::new("."));
            let agent = Arc::new(AgentRuntime::new(tool_executor));

            let sd_bin = config.storage.runtime_dir.join("windows-x64/sd-cpp/sd-cli.exe");
            let sd_model_dir = config.storage.models_dir.join("flux2-klein");
            let sd_worker = Arc::new(StableDiffusionWorker::new(sd_bin, sd_model_dir, config.storage.artifacts_dir.clone()));

            let (event_tx, _) = broadcast::channel(256);

            let jspace = Arc::new(j_space::JSpaceManager::new(config.jspace.max_active_sessions));
            let brain = Arc::new(brain::PersistentBrain::new(Arc::new(db.clone())));

            let state = api::AppState {
                config: config.clone(),
                db,
                registry,
                resource,
                telemetry,
                watchdog,
                agent,
                jspace,
                brain,
                active_workers: Arc::new(Mutex::new(HashMap::new())),
                sd_worker,
                event_tx,
            };

            let app = api::create_router(state);
            let addr = format!("{}:{}", config.server.host, listen_port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;

            info!("============================================================");
            info!("HCS Local AI Daemon v{} is running on http://{}", env!("CARGO_PKG_VERSION"), addr);
            info!("• Dashboard:          http://{}/", addr);
            info!("• OpenAI API:         http://{}/v1", addr);
            info!("• Anthropic API:      http://{}/v1/messages", addr);
            info!("• HCS Native API:     http://{}/hcs/v1", addr);
            info!("============================================================");

            axum::serve(listener, app).await?;
            Ok(())
        }
    }
}
