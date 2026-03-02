use std::sync::Arc;

use crate::memory::{MemoryKind, MemoryStore};
use crate::provider::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "core_memory_update".to_string(),
            description: "Update a core memory block that is always visible in your context. \
                          Use this to persist important facts about the user or yourself. \
                          Available blocks: 'human' (facts about the user) and 'agent' (your persona/preferences)."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "block": {
                        "type": "string",
                        "enum": ["human", "agent"],
                        "description": "Which core memory block to update"
                    },
                    "content": {
                        "type": "string",
                        "description": "The full new content for this block (replaces previous content)"
                    }
                },
                "required": ["block", "content"]
            }),
        },
        ToolDefinition {
            name: "memory_search".to_string(),
            description: "Search your long-term archival memory for relevant information. \
                          Returns ranked results matching the query."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 10)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "memory_add".to_string(),
            description: "Add a new entry to long-term archival memory. Use for facts, preferences, \
                          decisions, or project context worth remembering across sessions."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The memory to store"
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["fact", "preference", "decision", "project", "entity", "belief"],
                        "description": "Memory category (default: fact)"
                    },
                    "importance": {
                        "type": "number",
                        "description": "Importance 0.0-1.0 (default: 0.5)"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "memory_delete".to_string(),
            description: "Delete a memory from archival storage by ID.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Memory ID to delete"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "memory_list".to_string(),
            description: "List recent memories from archival storage, optionally filtered by kind."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["fact", "preference", "decision", "project", "entity", "belief"],
                        "description": "Filter by memory kind"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 20)"
                    }
                }
            }),
        },
    ]
}

pub fn handle(
    name: &str,
    input: &serde_json::Value,
    store: &Arc<MemoryStore>,
    conversation_id: &str,
) -> Option<(String, bool)> {
    match name {
        "core_memory_update" => {
            let block = input.get("block").and_then(|v| v.as_str()).unwrap_or("");
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match store.update_block(block, content) {
                Ok(()) => Some((format!("Updated core memory block '{block}'."), false)),
                Err(e) => Some((format!("Error: {e}"), true)),
            }
        }
        "memory_search" => {
            let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = input
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize;
            match store.search(query, limit) {
                Ok(results) => {
                    if results.is_empty() {
                        Some(("No memories found matching that query.".to_string(), false))
                    } else {
                        let text = results
                            .iter()
                            .map(|r| {
                                format!(
                                    "- [{}] (id={}, importance={:.2}, score={:.3}) {}",
                                    r.memory.kind,
                                    r.memory.id,
                                    r.memory.importance,
                                    r.score,
                                    r.memory.content,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        Some((format!("{} results:\n{text}", results.len()), false))
                    }
                }
                Err(e) => Some((format!("Search error: {e}"), true)),
            }
        }
        "memory_add" => {
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let kind = input
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("fact");
            let importance = input
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32;
            let kind = MemoryKind::parse(kind);
            match store.add(content, &kind, importance, Some(conversation_id)) {
                Ok(id) => Some((format!("Memory added (id={id})."), false)),
                Err(e) => Some((format!("Error: {e}"), true)),
            }
        }
        "memory_delete" => {
            let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("");
            match store.delete(id) {
                Ok(()) => Some((format!("Memory '{id}' deleted."), false)),
                Err(e) => Some((format!("Error: {e}"), true)),
            }
        }
        "memory_list" => {
            let kind = input
                .get("kind")
                .and_then(|v| v.as_str())
                .map(MemoryKind::parse);
            let limit = input
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(20) as usize;
            match store.list(kind.as_ref(), limit) {
                Ok(memories) => {
                    if memories.is_empty() {
                        Some(("No memories found.".to_string(), false))
                    } else {
                        let text = memories
                            .iter()
                            .map(|m| {
                                format!(
                                    "- [{}] (id={}, importance={:.2}) {}",
                                    m.kind, m.id, m.importance, m.content,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        Some((format!("{} memories:\n{text}", memories.len()), false))
                    }
                }
                Err(e) => Some((format!("Error: {e}"), true)),
            }
        }
        _ => None,
    }
}
