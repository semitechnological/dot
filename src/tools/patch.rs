use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::Tool;

pub struct ApplyPatchTool;

impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply search-and-replace patches to one or more files. Each patch specifies a file path, the exact text to find (old), and the replacement text (new). Use for precise multi-file edits."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File path to modify"
                            },
                            "old": {
                                "type": "string",
                                "description": "Exact text to find in the file"
                            },
                            "new": {
                                "type": "string",
                                "description": "Replacement text"
                            }
                        },
                        "required": ["path", "old", "new"]
                    },
                    "description": "Array of patches to apply"
                }
            },
            "required": ["patches"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let patches = input["patches"]
            .as_array()
            .context("Missing required parameter 'patches'")?;

        if patches.is_empty() {
            return Ok("No patches to apply.".to_string());
        }

        tracing::debug!("apply_patch: {} patches", patches.len());

        let mut results: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for patch in patches {
            let path = match patch["path"].as_str() {
                Some(p) => p,
                None => {
                    errors.push("patch missing 'path' field".to_string());
                    continue;
                }
            };
            let old = match patch["old"].as_str() {
                Some(o) => o,
                None => {
                    errors.push(format!("{}: missing 'old' field", path));
                    continue;
                }
            };
            let new = match patch["new"].as_str() {
                Some(n) => n,
                None => {
                    errors.push(format!("{}: missing 'new' field", path));
                    continue;
                }
            };

            if old.is_empty() && new.is_empty() {
                continue;
            }

            if old.is_empty() {
                if let Some(parent) = Path::new(path).parent()
                    && !parent.as_os_str().is_empty()
                {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::write(path, new) {
                    Ok(()) => results.push(format!("created {}", path)),
                    Err(e) => errors.push(format!("{}: {}", path, e)),
                }
                continue;
            }

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{}: {}", path, e));
                    continue;
                }
            };

            if !content.contains(old) {
                errors.push(format!("{}: 'old' text not found in file", path));
                continue;
            }

            let updated = content.replacen(old, new, 1);
            match fs::write(path, &updated) {
                Ok(()) => results.push(format!("patched {}", path)),
                Err(e) => errors.push(format!("{}: write failed: {}", path, e)),
            }
        }

        let mut output = String::new();
        if !results.is_empty() {
            output.push_str(&results.join("\n"));
        }
        if !errors.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("Errors:\n");
            output.push_str(&errors.join("\n"));
        }

        Ok(output)
    }
}
