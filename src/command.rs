use std::collections::HashMap;
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::CommandConfig;

pub struct SlashCommand {
    pub name: String,
    pub description: String,
    command: String,
    _timeout: u64,
}

impl SlashCommand {
    pub fn from_config(name: &str, cfg: &CommandConfig) -> Self {
        SlashCommand {
            name: name.to_string(),
            description: cfg.description.clone(),
            command: cfg.command.clone(),
            _timeout: cfg.timeout,
        }
    }

    pub fn execute(&self, args: &str, cwd: &str) -> Result<String> {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&self.command);
        cmd.env("DOT_COMMAND", &self.name);
        cmd.env("DOT_ARGS", args);
        cmd.env("DOT_CWD", cwd);
        cmd.current_dir(cwd);
        let output = cmd
            .output()
            .with_context(|| format!("command '{}' failed to execute", self.name))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            anyhow::bail!(
                "command '{}' exited with {}: {}",
                self.name,
                output.status,
                stderr
            );
        }
        Ok(stdout.to_string())
    }
}

pub struct CommandRegistry {
    commands: HashMap<String, SlashCommand>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        CommandRegistry {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: SlashCommand) {
        tracing::info!("Registered command: /{}", cmd.name);
        self.commands.insert(cmd.name.clone(), cmd);
    }

    pub fn execute(&self, name: &str, args: &str, cwd: &str) -> Result<String> {
        match self.commands.get(name) {
            Some(cmd) => cmd.execute(args, cwd),
            None => anyhow::bail!("unknown command: /{}", name),
        }
    }

    pub fn list(&self) -> Vec<(&str, &str)> {
        self.commands
            .values()
            .map(|c| (c.name.as_str(), c.description.as_str()))
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
