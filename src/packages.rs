use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub tools: HashMap<String, ManifestTool>,
    #[serde(default)]
    pub commands: HashMap<String, ManifestCommand>,
    #[serde(default)]
    pub hooks: HashMap<String, ManifestHook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestTool {
    pub description: String,
    pub command: String,
    #[serde(default = "default_schema")]
    pub schema: serde_json::Value,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestCommand {
    pub description: String,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestHook {
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "required": []
    })
}

fn default_timeout() -> u64 {
    30
}

pub struct InstalledExtension {
    pub manifest: ExtensionManifest,
    pub path: PathBuf,
}

fn extensions_dir() -> PathBuf {
    Config::config_dir().join("extensions")
}

pub fn discover() -> Vec<InstalledExtension> {
    let dir = extensions_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("extension.toml");
        if !manifest_path.exists() {
            continue;
        }
        match load_manifest(&manifest_path) {
            Ok(manifest) => {
                results.push(InstalledExtension {
                    manifest,
                    path: path.clone(),
                });
            }
            Err(e) => {
                tracing::warn!("Failed to load extension from {}: {}", path.display(), e);
            }
        }
    }
    results
}

fn load_manifest(path: &Path) -> Result<ExtensionManifest> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

/// Resolve a command path relative to the extension directory.
fn resolve_command(ext_path: &Path, command: &str) -> String {
    let script = ext_path.join(command);
    if script.exists() {
        script.to_string_lossy().to_string()
    } else {
        command.to_string()
    }
}

/// Convert discovered extensions into config entries that can be merged into the main config.
pub fn merge_into_config(config: &mut crate::config::Config) {
    for ext in discover() {
        let dir = &ext.path;
        for (name, tool) in &ext.manifest.tools {
            let key = format!("{}:{}", ext.manifest.name, name);
            config
                .custom_tools
                .entry(key)
                .or_insert_with(|| crate::config::CustomToolConfig {
                    description: tool.description.clone(),
                    command: resolve_command(dir, &tool.command),
                    schema: tool.schema.clone(),
                    timeout: tool.timeout,
                });
        }
        for (name, cmd) in &ext.manifest.commands {
            let key = format!("{}:{}", ext.manifest.name, name);
            config
                .commands
                .entry(key)
                .or_insert_with(|| crate::config::CommandConfig {
                    description: cmd.description.clone(),
                    command: resolve_command(dir, &cmd.command),
                    timeout: cmd.timeout,
                });
        }
        for (event_name, hook) in &ext.manifest.hooks {
            config
                .hooks
                .entry(event_name.clone())
                .or_insert_with(|| crate::config::HookConfig {
                    command: resolve_command(dir, &hook.command),
                    timeout: hook.timeout,
                });
        }
    }
}

pub fn install(source: &str) -> Result<String> {
    let dir = extensions_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating extensions dir {}", dir.display()))?;

    let name = source
        .rsplit('/')
        .next()
        .unwrap_or("extension")
        .trim_end_matches(".git");
    let target = dir.join(name);

    if target.exists() {
        bail!(
            "Extension '{}' already installed at {}",
            name,
            target.display()
        );
    }

    let output = Command::new("git")
        .args(["clone", "--depth", "1", source])
        .arg(&target)
        .output()
        .context("running git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git clone failed: {}", stderr.trim());
    }

    let manifest_path = target.join("extension.toml");
    if !manifest_path.exists() {
        let _ = std::fs::remove_dir_all(&target);
        bail!(
            "No extension.toml found in {}. Not a valid dot extension.",
            source
        );
    }

    let manifest = load_manifest(&manifest_path)?;
    Ok(format!(
        "Installed '{}' ({} tools, {} commands, {} hooks)",
        manifest.name,
        manifest.tools.len(),
        manifest.commands.len(),
        manifest.hooks.len(),
    ))
}

pub fn uninstall(name: &str) -> Result<String> {
    let target = extensions_dir().join(name);
    if !target.exists() {
        bail!("Extension '{}' not found", name);
    }
    std::fs::remove_dir_all(&target).with_context(|| format!("removing {}", target.display()))?;
    Ok(format!("Removed extension '{}'", name))
}

pub fn list() -> Vec<(String, String, PathBuf)> {
    discover()
        .into_iter()
        .map(|e| (e.manifest.name, e.manifest.description, e.path))
        .collect()
}
