use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

pub struct ToolExecutor {
    pub workspace_root: PathBuf,
}

impl ToolExecutor {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "file_read".to_string(),
                description: "Read the full contents of a file relative to the workspace root".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path to file" }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "file_write".to_string(),
                description: "Write content to a file in the workspace".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path to file" },
                        "content": { "type": "string", "description": "Text content to write" }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "file_edit".to_string(),
                description: "Replace exact text chunk in a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path to file" },
                        "target_text": { "type": "string", "description": "Exact text to replace" },
                        "replacement_text": { "type": "string", "description": "New text" }
                    },
                    "required": ["path", "target_text", "replacement_text"]
                }),
            },
            ToolDefinition {
                name: "command_exec".to_string(),
                description: "Execute a shell command (PowerShell / Command Prompt) in the workspace".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Command string to execute" }
                    },
                    "required": ["command"]
                }),
            },
        ]
    }

    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        match call.name.as_str() {
            "file_read" => {
                let path_str = call.arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let full_path = self.workspace_root.join(path_str);
                match tokio::fs::read_to_string(&full_path).await {
                    Ok(content) => ToolResult {
                        call_id: call.id.clone(),
                        success: true,
                        output: content,
                        error: None,
                    },
                    Err(e) => ToolResult {
                        call_id: call.id.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read file {:?}: {}", full_path, e)),
                    },
                }
            }
            "file_write" => {
                let path_str = call.arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = call.arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let full_path = self.workspace_root.join(path_str);

                if let Some(parent) = full_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }

                match tokio::fs::write(&full_path, content).await {
                    Ok(_) => ToolResult {
                        call_id: call.id.clone(),
                        success: true,
                        output: format!("Successfully wrote {} bytes to {:?}", content.len(), full_path),
                        error: None,
                    },
                    Err(e) => ToolResult {
                        call_id: call.id.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to write file {:?}: {}", full_path, e)),
                    },
                }
            }
            "file_edit" => {
                let path_str = call.arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let target = call.arguments.get("target_text").and_then(|v| v.as_str()).unwrap_or("");
                let replacement = call.arguments.get("replacement_text").and_then(|v| v.as_str()).unwrap_or("");
                let full_path = self.workspace_root.join(path_str);

                match tokio::fs::read_to_string(&full_path).await {
                    Ok(existing) => {
                        if !existing.contains(target) {
                            ToolResult {
                                call_id: call.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!("Target text was not found in {:?}", full_path)),
                            }
                        } else {
                            let updated = existing.replacen(target, replacement, 1);
                            match tokio::fs::write(&full_path, updated).await {
                                Ok(_) => ToolResult {
                                    call_id: call.id.clone(),
                                    success: true,
                                    output: format!("Successfully replaced target chunk in {:?}", full_path),
                                    error: None,
                                },
                                Err(e) => ToolResult {
                                    call_id: call.id.clone(),
                                    success: false,
                                    output: String::new(),
                                    error: Some(format!("Failed to save edit to {:?}: {}", full_path, e)),
                                },
                            }
                        }
                    }
                    Err(e) => ToolResult {
                        call_id: call.id.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read file {:?}: {}", full_path, e)),
                    },
                }
            }
            "command_exec" => {
                let cmd_str = call.arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
                #[cfg(target_os = "windows")]
                let mut cmd = Command::new("powershell.exe");
                #[cfg(target_os = "windows")]
                cmd.args(["-NoProfile", "-NonInteractive", "-Command", cmd_str]);

                #[cfg(not(target_os = "windows"))]
                let mut cmd = Command::new("sh");
                #[cfg(not(target_os = "windows"))]
                cmd.args(["-c", cmd_str]);

                cmd.current_dir(&self.workspace_root);
                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let combined = if stderr.is_empty() {
                            stdout
                        } else {
                            format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr)
                        };
                        ToolResult {
                            call_id: call.id.clone(),
                            success: output.status.success(),
                            output: combined,
                            error: if output.status.success() {
                                None
                            } else {
                                Some(format!("Command exited with status: {}", output.status))
                            },
                        }
                    }
                    Err(e) => ToolResult {
                        call_id: call.id.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to execute command: {}", e)),
                    },
                }
            }
            other => ToolResult {
                call_id: call.id.clone(),
                success: false,
                output: String::new(),
                error: Some(format!("Unknown tool: {}", other)),
            },
        }
    }
}
