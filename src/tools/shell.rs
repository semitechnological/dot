use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

use super::Tool;

const MAX_OUTPUT_CHARS: usize = 10_000;

pub struct RunCommandTool;

impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output. Use this to run build commands, tests, git operations, etc."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let command = input["command"]
            .as_str()
            .context("Missing required parameter 'command'")?;
        let working_dir = input["working_directory"].as_str();

        tracing::debug!("run_command: {}", command);

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(command);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd
            .output()
            .with_context(|| format!("Failed to execute command: {}", command))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let mut combined = format!("Exit code: {}\n", exit_code);

        if !stdout.is_empty() {
            combined.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !stdout.is_empty() {
                combined.push('\n');
            }
            combined.push_str("STDERR:\n");
            combined.push_str(&stderr);
        }

        if combined.len() > MAX_OUTPUT_CHARS {
            combined.truncate(MAX_OUTPUT_CHARS);
            combined.push_str(&format!(
                "\n... (output truncated at {} chars)",
                MAX_OUTPUT_CHARS
            ));
        }

        Ok(combined)
    }
}
