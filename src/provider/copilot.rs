use std::{collections::HashMap, future::Future, pin::Pin};

use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestMessageContentPartImage, ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionRequestUserMessageContentPart, ChatCompletionTool, ChatCompletionToolType,
        CreateChatCompletionRequest, FinishReason, FunctionCall, FunctionObject, ImageUrl,
    },
};
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;

use crate::provider::{
    ContentBlock, Message, Provider, Role, StopReason, StreamEvent, StreamEventType,
    ToolDefinition, Usage,
};

const COPILOT_API_BASE: &str = "https://api.githubcopilot.com";
const DEFAULT_MODEL: &str = "gpt-4o";

pub struct CopilotProvider {
    client: Client<OpenAIConfig>,
    model: String,
    cached_models: std::sync::Mutex<Option<Vec<String>>>,
    context_windows: std::sync::Mutex<HashMap<String, u32>>,
}

impl CopilotProvider {
    pub fn new(token: impl Into<String>, model: impl Into<String>) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(token)
            .with_api_base(COPILOT_API_BASE);
        Self {
            client: Client::with_config(config),
            model: model.into(),
            cached_models: std::sync::Mutex::new(None),
            context_windows: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn known_context_window(model: &str) -> u32 {
    if model.starts_with("o1") || model.starts_with("o3") || model.starts_with("o4") {
        return 200_000;
    }
    if model.starts_with("gpt-4o") || model.starts_with("gpt-4-turbo") {
        return 128_000;
    }
    if model.contains("claude") {
        return 200_000;
    }
    if model.starts_with("gpt-4") {
        return 8_192;
    }
    0
}

#[derive(Default)]
struct ToolCallAccum {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

fn convert_messages(
    messages: &[Message],
    system: Option<&str>,
) -> anyhow::Result<Vec<ChatCompletionRequestMessage>> {
    let mut result: Vec<ChatCompletionRequestMessage> = Vec::new();

    if let Some(sys) = system {
        result.push(ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(sys.to_string()),
                name: None,
            },
        ));
    }

    for msg in messages {
        match msg.role {
            Role::System => {
                let text = extract_text(&msg.content);
                result.push(ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(text),
                        name: None,
                    },
                ));
            }
            Role::User => {
                let mut tool_results: Vec<(String, String)> = Vec::new();
                let mut texts: Vec<String> = Vec::new();
                let mut images: Vec<(String, String)> = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(t) => texts.push(t.clone()),
                        ContentBlock::Image { media_type, data } => {
                            images.push((media_type.clone(), data.clone()));
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => {
                            tool_results.push((tool_use_id.clone(), content.clone()));
                        }
                        _ => {}
                    }
                }

                for (id, content) in tool_results {
                    result.push(ChatCompletionRequestMessage::Tool(
                        ChatCompletionRequestToolMessage {
                            content: ChatCompletionRequestToolMessageContent::Text(content),
                            tool_call_id: id,
                        },
                    ));
                }

                if !images.is_empty() {
                    let mut parts: Vec<ChatCompletionRequestUserMessageContentPart> = Vec::new();
                    if !texts.is_empty() {
                        parts.push(ChatCompletionRequestUserMessageContentPart::Text(
                            ChatCompletionRequestMessageContentPartText {
                                text: texts.join("\n"),
                            },
                        ));
                    }
                    for (media_type, data) in images {
                        parts.push(ChatCompletionRequestUserMessageContentPart::ImageUrl(
                            ChatCompletionRequestMessageContentPartImage {
                                image_url: ImageUrl {
                                    url: format!("data:{};base64,{}", media_type, data),
                                    detail: None,
                                },
                            },
                        ));
                    }
                    result.push(ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Array(parts),
                            name: None,
                        },
                    ));
                } else if !texts.is_empty() {
                    result.push(ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(
                                texts.join("\n"),
                            ),
                            name: None,
                        },
                    ));
                }
            }
            Role::Assistant => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<ChatCompletionMessageToolCall> = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(t) => text_parts.push(t.clone()),
                        ContentBlock::ToolUse { id, name, input } => {
                            tool_calls.push(ChatCompletionMessageToolCall {
                                id: id.clone(),
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: name.clone(),
                                    arguments: serde_json::to_string(input).unwrap_or_default(),
                                },
                            });
                        }
                        _ => {}
                    }
                }

                let content = if text_parts.is_empty() {
                    None
                } else {
                    Some(ChatCompletionRequestAssistantMessageContent::Text(
                        text_parts.join("\n"),
                    ))
                };

                result.push(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content,
                        name: None,
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        refusal: None,
                        ..Default::default()
                    },
                ));
            }
        }
    }

    Ok(result)
}

fn extract_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text(t) = b {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_tools(tools: &[ToolDefinition]) -> Vec<ChatCompletionTool> {
    tools
        .iter()
        .map(|t| ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: t.name.clone(),
                description: Some(t.description.clone()),
                parameters: Some(t.input_schema.clone()),
                strict: None,
            },
        })
        .collect()
}

fn map_finish(reason: &FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::EndTurn,
        FinishReason::Length => StopReason::MaxTokens,
        FinishReason::ToolCalls | FinishReason::FunctionCall => StopReason::ToolUse,
        FinishReason::ContentFilter => StopReason::StopSequence,
    }
}

impl Provider for CopilotProvider {
    fn name(&self) -> &str {
        "copilot"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn available_models(&self) -> Vec<String> {
        let cache = self.cached_models.lock().unwrap();
        cache.clone().unwrap_or_default()
    }

    fn context_window(&self) -> u32 {
        let cw = self.context_windows.lock().unwrap();
        cw.get(&self.model).copied().unwrap_or(0)
    }

    fn fetch_context_window(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<u32>> + Send + '_>> {
        Box::pin(async move {
            {
                let cw = self.context_windows.lock().unwrap();
                if let Some(&v) = cw.get(&self.model) {
                    return Ok(v);
                }
            }
            let v = known_context_window(&self.model);
            if v > 0 {
                let mut cw = self.context_windows.lock().unwrap();
                cw.insert(self.model.clone(), v);
            }
            Ok(v)
        })
    }

    fn fetch_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>> {
        let client = self.client.clone();
        Box::pin(async move {
            {
                let cache = self.cached_models.lock().unwrap();
                if let Some(ref models) = *cache {
                    return Ok(models.clone());
                }
            }

            let resp = client.models().list().await;

            match resp {
                Ok(list) => {
                    let mut models: Vec<String> = list.data.into_iter().map(|m| m.id).collect();
                    models.sort();
                    models.dedup();

                    if models.is_empty() {
                        models.push(DEFAULT_MODEL.to_string());
                    }

                    let mut cache = self.cached_models.lock().unwrap();
                    *cache = Some(models.clone());
                    Ok(models)
                }
                Err(e) => {
                    warn!("Failed to fetch Copilot models, using defaults: {e}");
                    let defaults = vec![DEFAULT_MODEL.to_string()];
                    let mut cache = self.cached_models.lock().unwrap();
                    *cache = Some(defaults.clone());
                    Ok(defaults)
                }
            }
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
        thinking_budget: u32,
    ) -> Pin<
        Box<dyn Future<Output = anyhow::Result<mpsc::UnboundedReceiver<StreamEvent>>> + Send + '_>,
    > {
        self.stream_with_model(
            &self.model,
            messages,
            system,
            tools,
            max_tokens,
            thinking_budget,
        )
    }

    fn stream_with_model(
        &self,
        model: &str,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
        _thinking_budget: u32,
    ) -> Pin<
        Box<dyn Future<Output = anyhow::Result<mpsc::UnboundedReceiver<StreamEvent>>> + Send + '_>,
    > {
        let messages = messages.to_vec();
        let system = system.map(String::from);
        let tools = tools.to_vec();
        let model = model.to_string();
        let client = self.client.clone();

        Box::pin(async move {
            let converted = convert_messages(&messages, system.as_deref())
                .context("converting messages for Copilot")?;
            let converted_tools = convert_tools(&tools);

            let request = CreateChatCompletionRequest {
                model: model.clone(),
                messages: converted,
                max_completion_tokens: Some(max_tokens),
                stream: Some(true),
                tools: if converted_tools.is_empty() {
                    None
                } else {
                    Some(converted_tools)
                },
                temperature: Some(1.0),
                ..Default::default()
            };

            let mut stream = client
                .chat()
                .create_stream(request)
                .await
                .context("creating Copilot stream")?;

            let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();

            tokio::spawn(async move {
                let mut tool_accum: HashMap<u32, ToolCallAccum> = HashMap::new();
                let mut total_output: u32 = 0;
                let mut stop: Option<StopReason> = None;

                let _ = tx.send(StreamEvent {
                    event_type: StreamEventType::MessageStart,
                });

                while let Some(result) = stream.next().await {
                    match result {
                        Err(e) => {
                            warn!("Copilot stream error: {e}");
                            let _ = tx.send(StreamEvent {
                                event_type: StreamEventType::Error(e.to_string()),
                            });
                            return;
                        }
                        Ok(response) => {
                            if let Some(usage) = response.usage {
                                total_output = usage.completion_tokens;
                            }

                            for choice in response.choices {
                                if let Some(reason) = &choice.finish_reason {
                                    stop = Some(map_finish(reason));
                                    if matches!(
                                        reason,
                                        FinishReason::ToolCalls | FinishReason::FunctionCall
                                    ) {
                                        for accum in tool_accum.values() {
                                            if accum.started {
                                                let _ = tx.send(StreamEvent {
                                                    event_type: StreamEventType::ToolUseEnd,
                                                });
                                            }
                                        }
                                        tool_accum.clear();
                                    }
                                }

                                let delta = choice.delta;

                                if let Some(content) = delta.content
                                    && !content.is_empty()
                                {
                                    let _ = tx.send(StreamEvent {
                                        event_type: StreamEventType::TextDelta(content),
                                    });
                                }

                                if let Some(chunks) = delta.tool_calls {
                                    for chunk in chunks {
                                        let idx = chunk.index;
                                        let entry = tool_accum.entry(idx).or_default();

                                        if let Some(id) = chunk.id
                                            && !id.is_empty()
                                        {
                                            entry.id = id;
                                        }

                                        if let Some(func) = chunk.function {
                                            if let Some(name) = func.name
                                                && !name.is_empty()
                                            {
                                                entry.name = name;
                                            }

                                            if !entry.started
                                                && !entry.id.is_empty()
                                                && !entry.name.is_empty()
                                            {
                                                let _ = tx.send(StreamEvent {
                                                    event_type: StreamEventType::ToolUseStart {
                                                        id: entry.id.clone(),
                                                        name: entry.name.clone(),
                                                    },
                                                });
                                                entry.started = true;
                                            }

                                            if let Some(args) = func.arguments
                                                && !args.is_empty()
                                            {
                                                entry.arguments.push_str(&args);
                                                let _ = tx.send(StreamEvent {
                                                    event_type: StreamEventType::ToolUseInputDelta(
                                                        args,
                                                    ),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for accum in tool_accum.values() {
                    if accum.started {
                        let _ = tx.send(StreamEvent {
                            event_type: StreamEventType::ToolUseEnd,
                        });
                    }
                }

                let _ = tx.send(StreamEvent {
                    event_type: StreamEventType::MessageEnd {
                        stop_reason: stop.unwrap_or(StopReason::EndTurn),
                        usage: Usage {
                            input_tokens: 0,
                            output_tokens: total_output,
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                        },
                    },
                });
            });

            Ok(rx)
        })
    }
}
