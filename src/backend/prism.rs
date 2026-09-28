use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::info;

pub struct PrismWorker {
    #[allow(dead_code)]
    pub model_id: String,
    pub port: u16,
    child: Option<Child>,
    http_client: reqwest::Client,
}

impl PrismWorker {
    pub async fn start(
        model_id: &str,
        runtime_bin: &Path,
        model_path: &Path,
        mmproj_path: Option<&Path>,
        port: u16,
        context_size: usize,
        gpu_layers: u32,
        threads: usize,
        batch_size: usize,
        ubatch_size: usize,
        flash_attn: &str,
        cache_type_k: &str,
        cache_type_v: &str,
    ) -> anyhow::Result<Self> {
        info!("Starting Prism worker for {} on port {} (threads: {}, batch: {}, KV: {}/{}, flash-attn: {})...",
            model_id, port, threads, batch_size, cache_type_k, cache_type_v, flash_attn);

        let abs_bin = std::fs::canonicalize(runtime_bin)
            .unwrap_or_else(|_| runtime_bin.to_path_buf());
        let abs_model = std::fs::canonicalize(model_path)
            .unwrap_or_else(|_| model_path.to_path_buf());

        let bin_dir = abs_bin.parent().unwrap_or(Path::new("."));

        let mut cmd = Command::new(&abs_bin);
        cmd.current_dir(bin_dir);
        cmd.arg("-m").arg(&abs_model);
        cmd.arg("--host").arg("127.0.0.1");
        cmd.arg("--port").arg(port.to_string());
        cmd.arg("-c").arg(context_size.to_string());
        cmd.arg("-ngl").arg(gpu_layers.to_string());
        cmd.arg("-t").arg(threads.to_string());
        cmd.arg("-tb").arg(threads.to_string());
        cmd.arg("-b").arg(batch_size.to_string());
        cmd.arg("-ub").arg(ubatch_size.to_string());
        cmd.arg("--flash-attn").arg(flash_attn);
        cmd.arg("--cache-type-k").arg(cache_type_k);
        cmd.arg("--cache-type-v").arg(cache_type_v);
        cmd.arg("--cont-batching");

        if let Some(mmproj) = mmproj_path {
            if mmproj.exists() {
                let abs_mmproj = std::fs::canonicalize(mmproj).unwrap_or_else(|_| mmproj.to_path_buf());
                cmd.arg("--mmproj").arg(abs_mmproj);
            }
        }

        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let child = cmd.spawn()?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;

        let mut worker = Self {
            model_id: model_id.to_string(),
            port,
            child: Some(child),
            http_client: client,
        };

        // Health check polling: up to 180 seconds (for cold HDD loads)
        let health_url = format!("http://127.0.0.1:{}/health", port);
        let mut healthy = false;
        for i in 0..360 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Some(ref mut c) = worker.child {
                if let Ok(Some(status)) = c.try_wait() {
                    anyhow::bail!("Prism worker process exited prematurely with status: {}", status);
                }
            }

            match worker.http_client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("Prism worker for {} is healthy (ready in ~{:.1}s)", model_id, (i as f32) * 0.5);
                    healthy = true;
                    break;
                }
                _ => {}
            }
        }

        if !healthy {
            worker.stop().await;
            anyhow::bail!("Prism worker failed to become healthy within 180 seconds");
        }

        Ok(worker)
    }

    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            info!("Stopping Prism worker on port {}...", self.port);
            let _ = child.kill().await;
            let _ = child.wait().await;
            // Grace period to allow Windows kernel to reclaim committed virtual memory pages
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
}

impl Drop for PrismWorker {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}
