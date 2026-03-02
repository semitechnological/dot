use std::sync::Arc;

use anyhow::{Context, Result};

use crate::memory::{MemoryKind, MemoryStore};
use crate::provider::{ContentBlock, Message, Provider, Role, StreamEventType};

const EXTRACT_SYSTEM: &str = "\
You are a memory extraction assistant for an AI coding agent. Your job is to identify important, durable facts from the conversation that are worth remembering across sessions.

Extract facts that would be useful in future conversations: user preferences, technical decisions, project context, workflow patterns, environment details, names, and tool configurations.

DO NOT extract:
- Transient task details (file contents being edited right now, current debugging steps)
- Information obvious from the codebase (imports, function signatures)
- Generic coding knowledge

You will receive:
1. Recent conversation messages
2. An existing memory snapshot (may be empty)

Return ONLY valid JSON with this structure:
{
  \"add\": [{\"content\": \"...\", \"kind\": \"fact|preference|decision|project|entity|belief\", \"importance\": 0.0-1.0}],
  \"update\": [{\"id\": \"existing-memory-id\", \"content\": \"updated text\", \"importance\": 0.0-1.0}],
  \"delete\": [\"memory-id-that-is-stale-or-contradicted\"]
}

Rules:
- Each memory should be a single, self-contained statement
- Prefer updating an existing memory over adding a duplicate
- Delete memories that are contradicted by new information
- importance: 0.9+ for core identity/preferences, 0.7-0.9 for project facts, 0.5-0.7 for contextual details, <0.5 for ephemeral
- Return {\"add\":[],\"update\":[],\"delete\":[]} if nothing worth remembering";

#[derive(Debug)]
pub struct ExtractionResult {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

pub async fn extract(
    messages: &[Message],
    provider: &dyn Provider,
    store: &Arc<MemoryStore>,
    conversation_id: &str,
) -> Result<ExtractionResult> {
    let conversation_text = format_messages(messages);
    if conversation_text.trim().is_empty() {
        return Ok(ExtractionResult {
            added: 0,
            updated: 0,
            deleted: 0,
        });
    }

    let snapshot = store.snapshot(50).unwrap_or_default();
    let snapshot_text = if snapshot.is_empty() {
        "No existing memories.".to_string()
    } else {
        snapshot
            .iter()
            .map(|m| {
                format!(
                    "- [{}] id={} importance={:.2}: {}",
                    m.kind, m.id, m.importance, m.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let prompt = format!(
        "## Recent Conversation\n{}\n\n## Existing Memory Snapshot\n{}\n\nExtract memories as JSON.",
        conversation_text, snapshot_text
    );

    let request = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text(prompt)],
    }];

    let mut rx = provider
        .stream(&request, Some(EXTRACT_SYSTEM), &[], 2048, 0)
        .await
        .context("starting extraction stream")?;

    let mut response = String::new();
    while let Some(event) = rx.recv().await {
        if let StreamEventType::TextDelta(text) = event.event_type {
            response.push_str(&text);
        }
    }

    let json = extract_json(&response).unwrap_or(&response);
    let ops: ExtractionOps = serde_json::from_str(json).unwrap_or_default();

    let mut added = 0usize;
    let mut updated = 0usize;
    let mut deleted = 0usize;

    for item in &ops.add {
        let kind = MemoryKind::parse(&item.kind);
        let importance = item.importance.clamp(0.0, 1.0);
        if item.content.len() < 5 {
            continue;
        }
        match store.add(&item.content, &kind, importance, Some(conversation_id)) {
            Ok(_) => added += 1,
            Err(e) => tracing::warn!("memory add failed: {e}"),
        }
    }

    for item in &ops.update {
        let importance = item.importance.clamp(0.0, 1.0);
        match store.update(&item.id, &item.content, importance) {
            Ok(()) => updated += 1,
            Err(e) => tracing::warn!("memory update failed for {}: {e}", item.id),
        }
    }

    for id in &ops.delete {
        match store.delete(id) {
            Ok(()) => deleted += 1,
            Err(e) => tracing::warn!("memory delete failed for {id}: {e}"),
        }
    }

    tracing::info!(
        "memory extraction: +{added} ~{updated} -{deleted} from conversation {conversation_id}"
    );

    Ok(ExtractionResult {
        added,
        updated,
        deleted,
    })
}

fn format_messages(messages: &[Message]) -> String {
    let tail = if messages.len() > 20 {
        &messages[messages.len() - 20..]
    } else {
        messages
    };
    let mut out = String::new();
    for msg in tail {
        let role = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => continue,
        };
        for block in &msg.content {
            match block {
                ContentBlock::Text(t) if !t.is_empty() => {
                    let truncated: String = t.chars().take(2000).collect();
                    out.push_str(&format!("{role}: {truncated}\n\n"));
                }
                ContentBlock::ToolUse { name, .. } => {
                    out.push_str(&format!("{role}: [used tool: {name}]\n\n"));
                }
                _ => {}
            }
        }
    }
    out
}

fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Default, serde::Deserialize)]
struct ExtractionOps {
    #[serde(default)]
    add: Vec<AddOp>,
    #[serde(default)]
    update: Vec<UpdateOp>,
    #[serde(default)]
    delete: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct AddOp {
    content: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_importance")]
    importance: f32,
}

#[derive(Debug, serde::Deserialize)]
struct UpdateOp {
    id: String,
    content: String,
    #[serde(default = "default_importance")]
    importance: f32,
}

fn default_kind() -> String {
    "fact".to_string()
}

fn default_importance() -> f32 {
    0.5
}
