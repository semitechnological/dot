pub mod extract;
pub mod tools;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::fmt;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::db::schema;

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryKind {
    Fact,
    Preference,
    Decision,
    Project,
    Entity,
    Belief,
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl MemoryKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Decision => "decision",
            Self::Project => "project",
            Self::Entity => "entity",
            Self::Belief => "belief",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "preference" => Self::Preference,
            "decision" => Self::Decision,
            "project" => Self::Project,
            "entity" => Self::Entity,
            "belief" => Self::Belief,
            _ => Self::Fact,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub kind: MemoryKind,
    pub importance: f32,
    pub access_count: u32,
    pub source_conversation_id: Option<String>,
    pub superseded_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct MemoryBlock {
    pub id: String,
    pub name: String,
    pub content: String,
    pub updated_at: String,
}

pub struct MemoryStore {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryStore {
    pub fn open() -> Result<Self> {
        let path = crate::config::Config::db_path();
        let conn = Connection::open(&path)
            .with_context(|| format!("opening memory db at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("enabling WAL mode")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(schema::CREATE_MEMORY_BLOCKS)
            .context("creating memory_blocks table")?;
        conn.execute_batch(schema::CREATE_MEMORIES)
            .context("creating memories table")?;
        conn.execute_batch(schema::CREATE_MEMORIES_FTS)
            .context("creating memories_fts table")?;
        conn.execute_batch(schema::CREATE_MEMORIES_TRIGGERS)
            .context("creating memories triggers")?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_blocks WHERE name = 'human'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if count == 0 {
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO memory_blocks (id, name, content, updated_at) VALUES (?1, 'human', '', ?2)",
                params![Uuid::new_v4().to_string(), now],
            ).context("creating default human block")?;
            conn.execute(
                "INSERT INTO memory_blocks (id, name, content, updated_at) VALUES (?1, 'agent', '', ?2)",
                params![Uuid::new_v4().to_string(), now],
            ).context("creating default agent block")?;
        }
        Ok(())
    }

    pub fn get_block(&self, name: &str) -> Result<MemoryBlock> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, content, updated_at FROM memory_blocks WHERE name = ?1",
            params![name],
            |row| {
                Ok(MemoryBlock {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    content: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .with_context(|| format!("getting memory block '{name}'"))
    }

    pub fn blocks(&self) -> Result<Vec<MemoryBlock>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, content, updated_at FROM memory_blocks ORDER BY name")
            .context("preparing blocks query")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(MemoryBlock {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    content: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .context("querying blocks")?;
        let mut blocks = Vec::new();
        for row in rows {
            blocks.push(row.context("reading block row")?);
        }
        Ok(blocks)
    }

    pub fn update_block(&self, name: &str, content: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE memory_blocks SET content = ?1, updated_at = ?2 WHERE name = ?3",
                params![content, now, name],
            )
            .context("updating memory block")?;
        if affected == 0 {
            bail!("no memory block named '{name}'");
        }
        Ok(())
    }

    pub fn add(
        &self,
        content: &str,
        kind: &MemoryKind,
        importance: f32,
        source: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (id, content, kind, importance, access_count, source_conversation_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7)",
            params![id, content, kind.as_str(), importance, source, now, now],
        )
        .context("adding memory")?;
        tracing::debug!("added memory {id}: {}", &content[..content.len().min(60)]);
        Ok(id)
    }

    pub fn update(&self, id: &str, content: &str, importance: f32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE memories SET content = ?1, importance = ?2, updated_at = ?3 WHERE id = ?4 AND superseded_by IS NULL",
                params![content, importance, now, id],
            )
            .context("updating memory")?;
        if affected == 0 {
            bail!("memory '{id}' not found or superseded");
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM memories WHERE id = ?1", params![id])
            .context("deleting memory")?;
        Ok(())
    }

    pub fn supersede(&self, old_id: &str, new_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET superseded_by = ?1 WHERE id = ?2",
            params![new_id, old_id],
        )
        .context("superseding memory")?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ScoredMemory>> {
        let conn = self.conn.lock().unwrap();
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("\"{}\"", w.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" OR ");
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.content, m.kind, m.importance, m.access_count,
                        m.source_conversation_id, m.superseded_by, m.created_at, m.updated_at,
                        (-bm25(memories_fts)) * 0.5
                        + m.importance * (0.95 / (1.0 + (julianday('now') - julianday(m.updated_at)) / 7.0)) * 0.35
                        + MIN(1.0, CAST(m.access_count AS REAL) / 10.0) * 0.15 AS score
                 FROM memories m
                 JOIN memories_fts ON memories_fts.rowid = m.rowid
                 WHERE memories_fts MATCH ?1
                   AND m.superseded_by IS NULL
                 ORDER BY score DESC LIMIT ?2",
            )
            .context("preparing memory search")?;
        let rows = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                Ok(ScoredMemory {
                    memory: Memory {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        kind: MemoryKind::parse(
                            row.get::<_, String>(2)?.as_str(),
                        ),
                        importance: row.get(3)?,
                        access_count: row.get::<_, i64>(4)? as u32,
                        source_conversation_id: row.get(5)?,
                        superseded_by: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    },
                    score: row.get(9)?,
                })
            })
            .context("executing memory search")?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.context("reading memory search row")?);
        }
        // bump access counts
        for r in &results {
            let _ = conn.execute(
                "UPDATE memories SET access_count = access_count + 1 WHERE id = ?1",
                params![r.memory.id],
            );
        }
        Ok(results)
    }

    pub fn list(&self, kind: Option<&MemoryKind>, limit: usize) -> Result<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();
        let (sql, kind_val);
        if let Some(k) = kind {
            kind_val = k.as_str().to_string();
            sql = "SELECT id, content, kind, importance, access_count, source_conversation_id, superseded_by, created_at, updated_at \
                   FROM memories WHERE kind = ?1 AND superseded_by IS NULL ORDER BY updated_at DESC LIMIT ?2";
        } else {
            kind_val = String::new();
            sql = "SELECT id, content, kind, importance, access_count, source_conversation_id, superseded_by, created_at, updated_at \
                   FROM memories WHERE superseded_by IS NULL ORDER BY updated_at DESC LIMIT ?2";
        }
        let mut stmt = conn.prepare(sql).context("preparing memory list")?;
        let rows = if kind.is_some() {
            stmt.query_map(params![kind_val, limit as i64], map_memory_row)
                .context("listing memories")?
        } else {
            stmt.query_map(params![kind_val, limit as i64], map_memory_row)
                .context("listing memories")?
        };
        let mut memories = Vec::new();
        for row in rows {
            memories.push(row.context("reading memory list row")?);
        }
        Ok(memories)
    }

    pub fn snapshot(&self, limit: usize) -> Result<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, content, kind, importance, access_count, source_conversation_id, superseded_by, created_at, updated_at \
                 FROM memories WHERE superseded_by IS NULL ORDER BY importance DESC, updated_at DESC LIMIT ?1",
            )
            .context("preparing memory snapshot")?;
        let rows = stmt
            .query_map(params![limit as i64], map_memory_row)
            .context("querying memory snapshot")?;
        let mut memories = Vec::new();
        for row in rows {
            memories.push(row.context("reading snapshot row")?);
        }
        Ok(memories)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE superseded_by IS NULL",
                [],
                |r| r.get(0),
            )
            .context("counting memories")?;
        Ok(count as usize)
    }

    pub fn inject_context(&self, query: &str, count: usize) -> Result<String> {
        let mut out = String::new();

        // Core blocks
        let blocks = self.blocks()?;
        let has_core = blocks.iter().any(|b| !b.content.is_empty());
        if has_core {
            out.push_str("<memory>\n## Core\n");
            for block in &blocks {
                if !block.content.is_empty() {
                    out.push_str(&format!("[{}]\n{}\n\n", block.name, block.content));
                }
            }
        }

        // Archival search
        let results = self.search(query, count)?;
        if !results.is_empty() {
            if !has_core {
                out.push_str("<memory>\n");
            }
            out.push_str("## Relevant Context\n");
            for r in &results {
                out.push_str(&format!(
                    "[{}] {} ({:.2})\n",
                    r.memory.kind, r.memory.content, r.memory.importance
                ));
            }
        }

        if !out.is_empty() {
            out.push_str("</memory>");
        }
        Ok(out)
    }
}

fn map_memory_row(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: row.get(0)?,
        content: row.get(1)?,
        kind: MemoryKind::parse(row.get::<_, String>(2)?.as_str()),
        importance: row.get(3)?,
        access_count: row.get::<_, i64>(4)? as u32,
        source_conversation_id: row.get(5)?,
        superseded_by: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

unsafe impl Send for MemoryStore {}
unsafe impl Sync for MemoryStore {}
