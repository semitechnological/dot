use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

use super::Tool;

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns file paths sorted by modification time (newest first). Use for locating files by name."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g. '**/*.rs', 'src/**/*.ts', '*.json')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory to search from (defaults to current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required parameter 'pattern'")?;
        let base = input["path"].as_str().unwrap_or(".");
        tracing::debug!("glob: {} in {}", pattern, base);

        let full = if Path::new(pattern).is_absolute() {
            pattern.to_string()
        } else {
            format!("{}/{}", base, pattern)
        };

        let mut entries: Vec<(String, std::time::SystemTime)> = Vec::new();
        for path in glob::glob(&full).with_context(|| format!("invalid glob: {}", pattern))? {
            if entries.len() >= 200 {
                break;
            }
            if let Ok(p) = path {
                let mtime = p
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                entries.push((p.display().to_string(), mtime));
            }
        }

        entries.sort_by(|a, b| b.1.cmp(&a.1));

        if entries.is_empty() {
            Ok(format!("No files matching '{}'", pattern))
        } else {
            let count = entries.len();
            let paths: Vec<String> = entries.into_iter().map(|(p, _)| p).collect();
            let mut output = paths.join("\n");
            if count >= 200 {
                output.push_str("\n... (truncated at 200 results)");
            }
            Ok(output)
        }
    }
}
