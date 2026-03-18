use crate::agent::AgentEvent;
use crate::provider::{ContentBlock, Message, Provider, Role, StreamEventType, ToolDefinition};
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

const SUBAGENT_SYSTEM_PROMPT: &str = "\
You are a focused subagent. Complete the assigned task autonomously using available tools. \
Do not ask questions — make decisions independently. Be thorough but concise in your final response. \
Return only essential findings and results.";

struct PendingCall {
    id: String,
    name: String,
    input: String,
}

impl super::Agent {
    /// Blocking subagent: runs inline, streams events, returns final text.
    pub(super) async fn run_subagent(
        &mut self,
        description: &str,
        task: &str,
        profile: Option<&str>,
        event_tx: &UnboundedSender<AgentEvent>,
    ) -> Result<String> {
        let id = format!("sub_{:x}", rand_id());

        let _ = event_tx.send(AgentEvent::SubagentStart {
            id: id.clone(),
            description: description.to_string(),
            background: false,
        });

        let (system_prompt, tool_filter) = if let Some(name) = profile
            && let Some(p) = self.profiles.iter().find(|p| p.name == name)
        {
            let prompt = format!("[Subagent Task: {}]\n\n{}", description, p.system_prompt);
            (prompt, p.tool_filter.clone())
        } else {
            let prompt = format!("{}\n\nTask: {}", SUBAGENT_SYSTEM_PROMPT, description);
            (prompt, std::collections::HashMap::new())
        };

        let system_prompt = self.agents_context.apply_to_system_prompt(&system_prompt);
        let max_turns = self.subagent_max_turns;

        let mut messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text(task.to_string())],
        }];

        let mut final_text = String::new();
        let model_override = self.subagent_model.clone();

        for _turn in 0..max_turns {
            let mut tool_defs: Vec<ToolDefinition> = self.tools.definitions_filtered(&tool_filter);
            tool_defs.retain(|t| t.name != "subagent" && t.name != "subagent_result");

            let mut stream_rx = if let Some(ref model) = model_override {
                self.provider()
                    .stream_with_model(model, &messages, Some(&system_prompt), &tool_defs, 8192, 0)
                    .await?
            } else {
                self.provider()
                    .stream(&messages, Some(&system_prompt), &tool_defs, 8192, 0)
                    .await?
            };

            let mut text = String::new();
            let mut tool_calls: Vec<PendingCall> = Vec::new();
            let mut current_input = String::new();

            while let Some(event) = stream_rx.recv().await {
                match event.event_type {
                    StreamEventType::TextDelta(delta) => {
                        text.push_str(&delta);
                        let _ = event_tx.send(AgentEvent::SubagentDelta {
                            id: id.clone(),
                            text: delta,
                        });
                    }
                    StreamEventType::ToolUseStart { id: tid, name } => {
                        current_input.clear();
                        tool_calls.push(PendingCall {
                            id: tid,
                            name,
                            input: String::new(),
                        });
                    }
                    StreamEventType::ToolUseInputDelta(delta) => {
                        current_input.push_str(&delta);
                    }
                    StreamEventType::ToolUseEnd => {
                        if let Some(tc) = tool_calls.last_mut() {
                            tc.input = current_input.clone();
                        }
                        current_input.clear();
                    }
                    _ => {}
                }
            }

            let mut content_blocks = Vec::new();
            if !text.is_empty() {
                content_blocks.push(ContentBlock::Text(text.clone()));
            }
            for tc in &tool_calls {
                let input_value: serde_json::Value =
                    serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);
                content_blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: input_value,
                });
            }
            messages.push(Message {
                role: Role::Assistant,
                content: content_blocks,
            });

            if tool_calls.is_empty() {
                final_text = text;
                break;
            }

            let mut result_blocks = Vec::new();
            for tc in &tool_calls {
                let input_value: serde_json::Value =
                    serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);

                let detail = crate::tui::tools::extract_tool_detail(&tc.name, &tc.input);
                let _ = event_tx.send(AgentEvent::SubagentToolStart {
                    id: id.clone(),
                    tool_name: tc.name.clone(),
                    detail: detail.clone(),
                });

                if tc.name == "write_file" || tc.name == "apply_patch" {
                    if tc.name == "write_file" {
                        if let Some(path) = input_value.get("path").and_then(|v| v.as_str()) {
                            self.snapshots.before_write(path);
                        }
                    } else if let Some(patches) =
                        input_value.get("patches").and_then(|v| v.as_array())
                    {
                        for patch in patches {
                            if let Some(path) = patch.get("path").and_then(|v| v.as_str()) {
                                self.snapshots.before_write(path);
                            }
                        }
                    }
                }

                let tool_name = tc.name.clone();
                let exec_result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                    tokio::task::block_in_place(|| self.tools.execute(&tool_name, input_value))
                })
                .await;

                let (output, is_error) = match exec_result {
                    Err(_) => (format!("Tool '{}' timed out after 30s.", tc.name), true),
                    Ok(Err(e)) => (e.to_string(), true),
                    Ok(Ok(out)) => (out, false),
                };

                let _ = event_tx.send(AgentEvent::SubagentToolComplete {
                    id: id.clone(),
                    tool_name: tc.name.clone(),
                });

                tracing::debug!(
                    "subagent tool '{}' (error={}): {}",
                    tc.name,
                    is_error,
                    &output[..output.len().min(200)]
                );

                result_blocks.push(ContentBlock::ToolResult {
                    tool_use_id: tc.id.clone(),
                    content: output,
                    is_error,
                });
            }

            messages.push(Message {
                role: Role::User,
                content: result_blocks,
            });

            final_text = text;
        }

        let _ = event_tx.send(AgentEvent::SubagentComplete {
            id,
            output: final_text.clone(),
        });

        if final_text.is_empty() {
            Ok("[subagent completed with no text output]".to_string())
        } else {
            Ok(final_text)
        }
    }

    /// Spawn a non-blocking background subagent.
    pub(super) fn spawn_background_subagent(
        &mut self,
        description: &str,
        task: &str,
        profile: Option<&str>,
    ) -> Result<String> {
        let tx = self
            .background_tx
            .clone()
            .ok_or_else(|| anyhow::anyhow!("background event channel not configured"))?;

        let id = format!("bg_{:x}", rand_id());

        let (system_prompt, tool_filter) = if let Some(name) = profile
            && let Some(p) = self.profiles.iter().find(|p| p.name == name)
        {
            let prompt = format!("[Subagent Task: {}]\n\n{}", description, p.system_prompt);
            (prompt, p.tool_filter.clone())
        } else {
            let prompt = format!("{}\n\nTask: {}", SUBAGENT_SYSTEM_PROMPT, description);
            (prompt, std::collections::HashMap::new())
        };

        let system_prompt = self.agents_context.apply_to_system_prompt(&system_prompt);

        let mut tool_defs: Vec<ToolDefinition> = self.tools.definitions_filtered(&tool_filter);
        tool_defs.retain(|t| t.name != "subagent" && t.name != "subagent_result");

        let ctx = BackgroundCtx {
            id: id.clone(),
            description: description.to_string(),
            task: task.to_string(),
            provider: self.provider_arc(),
            tools: Arc::clone(&self.tools),
            tool_defs,
            system_prompt,
            max_turns: self.subagent_max_turns,
            model_override: self.subagent_model.clone(),
            event_tx: tx,
            results: Arc::clone(&self.background_results),
        };

        let _ = ctx.event_tx.send(AgentEvent::SubagentStart {
            id: id.clone(),
            description: description.to_string(),
            background: true,
        });

        let handle = tokio::spawn(run_background(ctx));
        self.background_handles.insert(id.clone(), handle);

        Ok(id)
    }
}

struct BackgroundCtx {
    id: String,
    description: String,
    task: String,
    provider: Arc<dyn Provider>,
    tools: Arc<ToolRegistry>,
    tool_defs: Vec<ToolDefinition>,
    system_prompt: String,
    max_turns: usize,
    model_override: Option<String>,
    event_tx: UnboundedSender<AgentEvent>,
    results: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

async fn run_background(ctx: BackgroundCtx) {
    let result = run_background_inner(&ctx).await;
    let output = match result {
        Ok(text) => text,
        Err(e) => {
            tracing::error!("background subagent {} error: {e}", ctx.id);
            format!("[subagent error: {e}]")
        }
    };

    {
        let mut results = ctx.results.lock().unwrap_or_else(|e| e.into_inner());
        results.insert(ctx.id.clone(), output.clone());
    }

    let _ = ctx.event_tx.send(AgentEvent::SubagentComplete {
        id: ctx.id.clone(),
        output: output.clone(),
    });
    let _ = ctx.event_tx.send(AgentEvent::SubagentBackgroundDone {
        id: ctx.id,
        description: ctx.description,
        output,
    });
}

async fn run_background_inner(ctx: &BackgroundCtx) -> Result<String> {
    let mut messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text(ctx.task.clone())],
    }];

    let mut final_text = String::new();

    for _turn in 0..ctx.max_turns {
        let mut stream_rx = if let Some(ref model) = ctx.model_override {
            ctx.provider
                .stream_with_model(
                    model,
                    &messages,
                    Some(&ctx.system_prompt),
                    &ctx.tool_defs,
                    8192,
                    0,
                )
                .await?
        } else {
            ctx.provider
                .stream(&messages, Some(&ctx.system_prompt), &ctx.tool_defs, 8192, 0)
                .await?
        };

        let mut text = String::new();
        let mut tool_calls: Vec<PendingCall> = Vec::new();
        let mut current_input = String::new();

        while let Some(event) = stream_rx.recv().await {
            match event.event_type {
                StreamEventType::TextDelta(delta) => {
                    text.push_str(&delta);
                    let _ = ctx.event_tx.send(AgentEvent::SubagentDelta {
                        id: ctx.id.clone(),
                        text: delta,
                    });
                }
                StreamEventType::ToolUseStart { id, name } => {
                    current_input.clear();
                    tool_calls.push(PendingCall {
                        id,
                        name,
                        input: String::new(),
                    });
                }
                StreamEventType::ToolUseInputDelta(delta) => {
                    current_input.push_str(&delta);
                }
                StreamEventType::ToolUseEnd => {
                    if let Some(tc) = tool_calls.last_mut() {
                        tc.input = current_input.clone();
                    }
                    current_input.clear();
                }
                _ => {}
            }
        }

        let mut content_blocks = Vec::new();
        if !text.is_empty() {
            content_blocks.push(ContentBlock::Text(text.clone()));
        }
        for tc in &tool_calls {
            let input_value: serde_json::Value =
                serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);
            content_blocks.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: input_value,
            });
        }
        messages.push(Message {
            role: Role::Assistant,
            content: content_blocks,
        });

        if tool_calls.is_empty() {
            final_text = text;
            break;
        }

        let mut result_blocks = Vec::new();
        for tc in &tool_calls {
            let input_value: serde_json::Value =
                serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);

            let detail = crate::tui::tools::extract_tool_detail(&tc.name, &tc.input);
            let _ = ctx.event_tx.send(AgentEvent::SubagentToolStart {
                id: ctx.id.clone(),
                tool_name: tc.name.clone(),
                detail,
            });

            let tool_name = tc.name.clone();
            let exec_result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                tokio::task::block_in_place(|| ctx.tools.execute(&tool_name, input_value))
            })
            .await;

            let (output, is_error) = match exec_result {
                Err(_) => (format!("Tool '{}' timed out after 30s.", tc.name), true),
                Ok(Err(e)) => (e.to_string(), true),
                Ok(Ok(out)) => (out, false),
            };

            let _ = ctx.event_tx.send(AgentEvent::SubagentToolComplete {
                id: ctx.id.clone(),
                tool_name: tc.name.clone(),
            });

            tracing::debug!(
                "bg subagent tool '{}' (error={}): {}",
                tc.name,
                is_error,
                &output[..output.len().min(200)]
            );

            result_blocks.push(ContentBlock::ToolResult {
                tool_use_id: tc.id.clone(),
                content: output,
                is_error,
            });
        }

        messages.push(Message {
            role: Role::User,
            content: result_blocks,
        });

        final_text = text;
    }

    if final_text.is_empty() {
        Ok("[subagent completed with no text output]".to_string())
    } else {
        Ok(final_text)
    }
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (t ^ (t >> 32)) as u64
}
