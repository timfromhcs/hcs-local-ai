use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info};
use uuid::Uuid;

pub struct StableDiffusionWorker {
    pub cli_bin: PathBuf,
    pub model_dir: PathBuf,
    pub artifacts_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ImageGenerateParams {
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub seed: i64,
    pub ref_image_path: Option<PathBuf>,
}

impl StableDiffusionWorker {
    pub fn new(cli_bin: PathBuf, model_dir: PathBuf, artifacts_dir: PathBuf) -> Self {
        Self {
            cli_bin,
            model_dir,
            artifacts_dir,
        }
    }

    pub async fn generate_image(&self, params: ImageGenerateParams) -> anyhow::Result<PathBuf> {
        let diff_model = self.model_dir.join("bonsai-flux2-klein-ternary-q2_k.gguf");
        let llm_model = self.model_dir.join("Qwen3-4B-Q4_K_M.gguf");
        let vae_model = self.model_dir.join("full_encoder_small_decoder.safetensors");

        if !diff_model.exists() {
            anyhow::bail!("Diffusion model not found: {:?}", diff_model);
        }
        if !llm_model.exists() {
            anyhow::bail!("Text encoder LLM model not found: {:?}", llm_model);
        }
        if !vae_model.exists() {
            anyhow::bail!("VAE model not found: {:?}", vae_model);
        }

        let out_id = Uuid::new_v4().to_string();
        let out_filename = format!("gen_{}.png", out_id);
        let out_path = self.artifacts_dir.join(&out_filename);

        info!("Executing stable-diffusion.cpp Vulkan T2I/I2I for prompt: '{}' -> {:?}", params.prompt, out_path);

        let mut cmd = Command::new(&self.cli_bin);
        cmd.arg("--diffusion-model").arg(&diff_model);
        cmd.arg("--llm").arg(&llm_model);
        cmd.arg("--vae").arg(&vae_model);
        cmd.arg("--vae-format").arg("flux2");
        cmd.arg("-p").arg(&params.prompt);
        cmd.arg("-o").arg(&out_path);
        cmd.arg("-W").arg(params.width.to_string());
        cmd.arg("-H").arg(params.height.to_string());
        cmd.arg("--steps").arg(params.steps.to_string());
        cmd.arg("-s").arg(params.seed.to_string());

        if let Some(ref_img) = params.ref_image_path {
            if ref_img.exists() {
                cmd.arg("-r").arg(ref_img);
            }
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("stable-diffusion.cpp execution failed: {}", stderr);
            anyhow::bail!("stable-diffusion.cpp failed with status {}: {}", output.status, stderr);
        }

        if !out_path.exists() {
            anyhow::bail!("Expected output image was not created at {:?}", out_path);
        }

        info!("Image generated successfully: {:?}", out_path);
        Ok(out_path)
    }
}
