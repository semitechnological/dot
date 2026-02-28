use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::provider::ToolDefinition;
use crate::tools::Tool;

// ============================================================================
// Lifecycle Events — mirrors pi's 30+ event system via config hooks
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    SessionStart,
    SessionEnd,
    BeforePrompt,
    AfterPrompt,
    BeforeToolCall,
    AfterToolCall,
    BeforeCompact,
    AfterCompact,
    ModelSwitch,
    AgentSwitch,
    OnError,
    OnStreamStart,
    OnStreamEnd,
    OnResume,
    OnUserInput,
    OnToolError,
    BeforeExit,
    OnThinkingStart,
    OnThinkingEnd,
    OnTitleGenerated,
    BeforePermissionCheck,
    OnContextLoad,
}

impl FromStr for Event {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "session_start" => Ok(Self::SessionStart),
            "session_end" => Ok(Self::SessionEnd),
            "before_prompt" => Ok(Self::BeforePrompt),
            "after_prompt" => Ok(Self::AfterPrompt),
            "before_tool_call" => Ok(Self::BeforeToolCall),
            "after_tool_call" => Ok(Self::AfterToolCall),
            "before_compact" => Ok(Self::BeforeCompact),
            "after_compact" => Ok(Self::AfterCompact),
            "model_switch" => Ok(Self::ModelSwitch),
            "agent_switch" => Ok(Self::AgentSwitch),
            "on_error" => Ok(Self::OnError),
            "on_stream_start" => Ok(Self::OnStreamStart),
            "on_stream_end" => Ok(Self::OnStreamEnd),
            "on_resume" => Ok(Self::OnResume),
            "on_user_input" => Ok(Self::OnUserInput),
            "on_tool_error" => Ok(Self::OnToolError),
            "before_exit" => Ok(Self::BeforeExit),
            "on_thinking_start" => Ok(Self::OnThinkingStart),
            "on_thinking_end" => Ok(Self::OnThinkingEnd),
            "on_title_generated" => Ok(Self::OnTitleGenerated),
            "before_permission_check" => Ok(Self::BeforePermissionCheck),
            "on_context_load" => Ok(Self::OnContextLoad),
            _ => Err(()),
        }
    }
}

impl Event {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::BeforePrompt => "before_prompt",
            Self::AfterPrompt => "after_prompt",
            Self::BeforeToolCall => "before_tool_call",
            Self::AfterToolCall => "after_tool_call",
            Self::BeforeCompact => "before_compact",
            Self::AfterCompact => "after_compact",
            Self::ModelSwitch => "model_switch",
            Self::AgentSwitch => "agent_switch",
            Self::OnError => "on_error",
            Self::OnStreamStart => "on_stream_start",
            Self::OnStreamEnd => "on_stream_end",
            Self::OnResume => "on_resume",
            Self::OnUserInput => "on_user_input",
            Self::OnToolError => "on_tool_error",
            Self::BeforeExit => "before_exit",
            Self::OnThinkingStart => "on_thinking_start",
            Self::OnThinkingEnd => "on_thinking_end",
            Self::OnTitleGenerated => "on_title_generated",
            Self::BeforePermissionCheck => "before_permission_check",
            Self::OnContextLoad => "on_context_load",
        }
    }

    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::BeforePrompt
                | Self::BeforeToolCall
                | Self::BeforeCompact
                | Self::BeforePermissionCheck
        )
    }
}

// ============================================================================
// Event Context — data passed to hook handlers
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct EventContext {
    pub event: String,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub session_id: String,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub prompt: Option<String>,
    pub error: Option<String>,
    pub title: Option<String>,
    pub agent_name: Option<String>,
}

// ============================================================================
// HookResult — what a hook returns (allow, block, or modify)
// ============================================================================

#[derive(Debug, Clone)]
pub enum HookResult {
    /// Hook executed successfully, proceed normally
    Allow,
    /// Hook wants to block the action (before_* events only)
    Block(String),
    /// Hook wants to modify the data (stdout contents replace the input)
    Modify(String),
}

// ============================================================================
// Hook — a shell command triggered on a lifecycle event
// ============================================================================

#[derive(Debug, Clone)]
pub struct Hook {
    pub event: Event,
    pub command: String,
    pub timeout: u64,
}

impl Hook {
    pub fn execute(&self, ctx: &EventContext) -> Result<HookResult> {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&self.command);
        cmd.env("DOT_EVENT", &ctx.event);
        cmd.env("DOT_MODEL", &ctx.model);
        cmd.env("DOT_PROVIDER", &ctx.provider);
        cmd.env("DOT_CWD", &ctx.cwd);
        cmd.env("DOT_SESSION_ID", &ctx.session_id);
        if let Some(ref name) = ctx.tool_name {
            cmd.env("DOT_TOOL_NAME", name);
        }
        if let Some(ref input) = ctx.tool_input {
            cmd.env("DOT_TOOL_INPUT", input);
        }
        if let Some(ref output) = ctx.tool_output {
            cmd.env("DOT_TOOL_OUTPUT", output);
        }
        if let Some(ref prompt) = ctx.prompt {
            cmd.env("DOT_PROMPT", prompt);
        }
        if let Some(ref error) = ctx.error {
            cmd.env("DOT_ERROR", error);
        }
        if let Some(ref title) = ctx.title {
            cmd.env("DOT_TITLE", title);
        }
        if let Some(ref agent) = ctx.agent_name {
            cmd.env("DOT_AGENT", agent);
        }
        let output = cmd
            .output()
            .with_context(|| format!("hook '{}' failed to execute", self.command))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Ok(HookResult::Block(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            return Ok(HookResult::Allow);
        }

        if self.event.is_blocking() {
            Ok(HookResult::Modify(stdout))
        } else {
            Ok(HookResult::Allow)
        }
    }
}

// ============================================================================
// Extension Trait — for compiled-in Rust extensions
// ============================================================================

pub trait Extension: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    fn tools(&self) -> Vec<Box<dyn Tool>> {
        Vec::new()
    }

    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }

    fn on_event(&self, _event: &Event, _ctx: &EventContext) -> Result<Option<String>> {
        Ok(None)
    }

    fn on_tool_call(&self, _name: &str, _input: Value) -> Result<String> {
        bail!("tool not implemented")
    }
}

// ============================================================================
// ScriptTool — a tool defined in config backed by a shell command
// ============================================================================

pub struct ScriptTool {
    tool_name: String,
    tool_description: String,
    schema: Value,
    command: String,
    _timeout: u64,
}

impl ScriptTool {
    pub fn new(
        name: String,
        description: String,
        schema: Value,
        command: String,
        timeout: u64,
    ) -> Self {
        ScriptTool {
            tool_name: name,
            tool_description: description,
            schema,
            command,
            _timeout: timeout,
        }
    }
}

impl Tool for ScriptTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn execute(&self, input: Value) -> Result<String> {
        let input_json = serde_json::to_string(&input)?;
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&self.command);
        cmd.env("DOT_TOOL_INPUT", &input_json);

        if let Some(obj) = input.as_object() {
            for (key, val) in obj {
                let env_key = format!("DOT_ARG_{}", key.to_uppercase());
                let env_val = match val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                cmd.env(env_key, env_val);
            }
        }

        let output = cmd
            .output()
            .with_context(|| format!("script tool '{}' failed", self.tool_name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            bail!(
                "script tool '{}' exited with {}: {}",
                self.tool_name,
                output.status,
                stderr
            );
        }

        Ok(stdout.to_string())
    }
}

// ============================================================================
// HookRegistry — manages lifecycle hooks from config
// ============================================================================

pub struct HookRegistry {
    hooks: HashMap<Event, Vec<Hook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        HookRegistry {
            hooks: HashMap::new(),
        }
    }

    pub fn register(&mut self, hook: Hook) {
        self.hooks.entry(hook.event.clone()).or_default().push(hook);
    }

    /// Fire-and-forget emit for non-blocking events.
    pub fn emit(&self, event: &Event, ctx: &EventContext) {
        if let Some(hooks) = self.hooks.get(event) {
            for hook in hooks {
                match hook.execute(ctx) {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("hook for '{}' failed: {}", event.as_str(), e);
                    }
                }
            }
        }
    }

    /// Blocking emit for before_* events. Returns Block if any hook blocks,
    /// Modify with the last modifier's output, or Allow.
    pub fn emit_blocking(&self, event: &Event, ctx: &EventContext) -> HookResult {
        if let Some(hooks) = self.hooks.get(event) {
            let mut last_modify: Option<String> = None;
            for hook in hooks {
                match hook.execute(ctx) {
                    Ok(HookResult::Block(reason)) => {
                        tracing::info!("hook blocked '{}': {}", event.as_str(), reason.trim());
                        return HookResult::Block(reason);
                    }
                    Ok(HookResult::Modify(data)) => {
                        last_modify = Some(data);
                    }
                    Ok(HookResult::Allow) => {}
                    Err(e) => {
                        tracing::warn!("hook for '{}' failed: {}", event.as_str(), e);
                    }
                }
            }
            if let Some(data) = last_modify {
                return HookResult::Modify(data);
            }
        }
        HookResult::Allow
    }

    pub fn has_hooks(&self, event: &Event) -> bool {
        self.hooks.get(event).is_some_and(|h| !h.is_empty())
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ExtensionRegistry — manages compiled extensions
// ============================================================================

pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn Extension>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        ExtensionRegistry {
            extensions: Vec::new(),
        }
    }

    pub fn register(&mut self, ext: Box<dyn Extension>) {
        tracing::info!("Registered extension: {}", ext.name());
        self.extensions.push(ext);
    }

    pub fn tools(&self) -> Vec<Box<dyn Tool>> {
        self.extensions.iter().flat_map(|e| e.tools()).collect()
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.extensions
            .iter()
            .flat_map(|e| e.tool_definitions())
            .collect()
    }

    pub fn emit(&self, event: &Event, ctx: &EventContext) {
        for ext in &self.extensions {
            if let Err(e) = ext.on_event(event, ctx) {
                tracing::warn!(
                    "extension '{}' error on '{}': {}",
                    ext.name(),
                    event.as_str(),
                    e
                );
            }
        }
    }

    pub fn handle_tool_call(&self, name: &str, input: Value) -> Option<Result<String>> {
        for ext in &self.extensions {
            let defs = ext.tool_definitions();
            if defs.iter().any(|d| d.name == name) {
                return Some(ext.on_tool_call(name, input));
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
