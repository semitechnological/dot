use anyhow::{Context, Result};
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::Tool;

const MAX_RESULTS: usize = 100;

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. Returns matching lines with file paths and line numbers. More precise than search_files."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in"
                },
                "include": {
                    "type": "string",
                    "description": "File glob filter (e.g. '*.rs', '*.{ts,tsx}')"
                }
            },
            "required": ["pattern", "path"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required parameter 'pattern'")?;
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        let include = input["include"].as_str().unwrap_or("");
        tracing::debug!("grep: '{}' in {}", pattern, path);

        let re = Regex::new(pattern).with_context(|| format!("invalid regex: {}", pattern))?;

        let mut results = Vec::new();
        grep_recursive(Path::new(path), &re, include, &mut results);

        if results.is_empty() {
            Ok(format!("No matches for '{}' in '{}'", pattern, path))
        } else {
            let truncated = results.len() >= MAX_RESULTS;
            let mut output = results.join("\n");
            if truncated {
                output.push_str(&format!("\n... (truncated at {} matches)", MAX_RESULTS));
            }
            Ok(output)
        }
    }
}

fn grep_recursive(dir: &Path, re: &Regex, include: &str, results: &mut Vec<String>) {
    if results.len() >= MAX_RESULTS {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        if results.len() >= MAX_RESULTS {
            return;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "__pycache__"
                || name == ".git"
            {
                continue;
            }
            grep_recursive(&path, re, include, results);
        } else if metadata.is_file() {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !include.is_empty() && !matches_include(&name, include) {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (i, line) in content.lines().enumerate() {
                if results.len() >= MAX_RESULTS {
                    return;
                }
                if re.is_match(line) {
                    results.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
}

fn matches_include(filename: &str, include: &str) -> bool {
    if let Some(ext_pat) = include.strip_prefix("*.") {
        if ext_pat.starts_with('{') && ext_pat.ends_with('}') {
            let inner = &ext_pat[1..ext_pat.len() - 1];
            return inner
                .split(',')
                .any(|ext| filename.ends_with(&format!(".{}", ext.trim())));
        }
        return filename.ends_with(&format!(".{}", ext_pat));
    }
    filename == include
}
