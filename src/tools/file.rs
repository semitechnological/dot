use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::Tool;

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Use this to examine existing files."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to read"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        tracing::debug!("read_file: {}", path);
        let content =
            fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))?;
        Ok(content)
    }
}

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to write to"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        let content = input["content"]
            .as_str()
            .context("Missing required parameter 'content'")?;
        tracing::debug!("write_file: {}", path);

        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for: {}", path))?;
        }

        fs::write(path, content).with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path
        ))
    }
}

pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the contents of a directory."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        tracing::debug!("list_directory: {}", path);

        let read_dir =
            fs::read_dir(path).with_context(|| format!("Failed to read directory: {}", path))?;

        let mut entries: Vec<String> = Vec::new();
        for entry in read_dir {
            let entry = entry.context("Failed to read directory entry")?;
            let metadata = entry.metadata().context("Failed to read entry metadata")?;
            let kind = if metadata.is_dir() { "dir" } else { "file" };
            let size = if metadata.is_file() {
                metadata.len()
            } else {
                0
            };
            let name = entry.file_name().to_string_lossy().to_string();

            #[cfg(unix)]
            let perms = {
                use std::os::unix::fs::PermissionsExt;
                format!("{:o}", metadata.permissions().mode() & 0o777)
            };
            #[cfg(not(unix))]
            let perms = String::from("---");

            entries.push(format!("{:<5}  {:>10}  {}  {}", kind, size, perms, name));
        }

        entries.sort();

        if entries.is_empty() {
            Ok(format!("Directory '{}' is empty.", path))
        } else {
            Ok(format!("Contents of '{}':\n{}", path, entries.join("\n")))
        }
    }
}

pub struct SearchFilesTool;

impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files within a directory. Returns matching lines with file paths and line numbers."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory to search in"
                },
                "pattern": {
                    "type": "string",
                    "description": "The text pattern to search for"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.rs')"
                }
            },
            "required": ["path", "pattern"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        let pattern = input["pattern"]
            .as_str()
            .context("Missing required parameter 'pattern'")?;
        let file_pattern = input["file_pattern"].as_str().unwrap_or("");
        tracing::debug!("search_files: {} for '{}'", path, pattern);

        let mut results: Vec<String> = Vec::new();
        search_recursive(Path::new(path), pattern, file_pattern, &mut results, 50)?;

        if results.is_empty() {
            Ok(format!("No matches found for '{}' in '{}'.", pattern, path))
        } else {
            let truncated = results.len() >= 50;
            let mut output = results.join("\n");
            if truncated {
                output.push_str("\n... (output truncated at 50 matches)");
            }
            Ok(output)
        }
    }
}

fn search_recursive(
    dir: &Path,
    pattern: &str,
    file_pattern: &str,
    results: &mut Vec<String>,
    max: usize,
) -> Result<()> {
    if results.len() >= max {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        if results.len() >= max {
            return Ok(());
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let path = entry.path();

        if metadata.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
                continue;
            }
            search_recursive(&path, pattern, file_pattern, results, max)?;
        } else if metadata.is_file() {
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if !file_pattern.is_empty() && !matches_file_pattern(&file_name, file_pattern) {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                if results.len() >= max {
                    return Ok(());
                }
                if line.contains(pattern) {
                    results.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        line_num + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    Ok(())
}

fn matches_file_pattern(filename: &str, pattern: &str) -> bool {
    if let Some(ext_pattern) = pattern.strip_prefix("*.") {
        filename.ends_with(&format!(".{}", ext_pattern))
    } else {
        filename == pattern
    }
}
