use std::collections::HashMap;
use std::path::PathBuf;

pub struct SnapshotManager {
    originals: HashMap<PathBuf, Option<String>>,
    checkpoints: Vec<HashMap<PathBuf, Option<String>>>,
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotManager {
    pub fn new() -> Self {
        Self {
            originals: HashMap::new(),
            checkpoints: Vec::new(),
        }
    }

    pub fn before_write(&mut self, path: &str) {
        let path = PathBuf::from(path);
        if self.originals.contains_key(&path) {
            return;
        }
        let content = std::fs::read_to_string(&path).ok();
        self.originals.insert(path, content);
    }

    pub fn list_changes(&self) -> Vec<(String, ChangeKind)> {
        let mut changes = Vec::new();
        for (path, original) in &self.originals {
            let current = std::fs::read_to_string(path).ok();
            let kind = match (original, &current) {
                (None, Some(_)) => ChangeKind::Created,
                (Some(_), None) => ChangeKind::Deleted,
                (Some(old), Some(new)) if old != new => ChangeKind::Modified,
                (None, None) => continue,
                _ => continue,
            };
            changes.push((path.display().to_string(), kind));
        }
        changes.sort_by(|a, b| a.0.cmp(&b.0));
        changes
    }

    pub fn restore(&self, path: &str) -> anyhow::Result<String> {
        let key = PathBuf::from(path);
        let original = self
            .originals
            .get(&key)
            .ok_or_else(|| anyhow::anyhow!("no snapshot for {}", path))?;
        match original {
            None => {
                if key.exists() {
                    std::fs::remove_file(&key)?;
                    Ok(format!("deleted {} (was created this session)", path))
                } else {
                    Ok(format!("{} already gone", path))
                }
            }
            Some(content) => {
                std::fs::write(&key, content)?;
                Ok(format!("restored {}", path))
            }
        }
    }

    pub fn restore_all(&self) -> anyhow::Result<Vec<String>> {
        let mut restored = Vec::new();
        for (path, original) in &self.originals {
            match original {
                None => {
                    if path.exists() {
                        std::fs::remove_file(path)?;
                        restored.push(format!("deleted {}", path.display()));
                    }
                }
                Some(content) => {
                    std::fs::write(path, content)?;
                    restored.push(format!("restored {}", path.display()));
                }
            }
        }
        Ok(restored)
    }

    pub fn file_count(&self) -> usize {
        self.originals.len()
    }

    pub fn clear(&mut self) {
        self.originals.clear();
        self.checkpoints.clear();
    }

    pub fn checkpoint(&mut self) {
        let mut snap = HashMap::new();
        for path in self.originals.keys() {
            let content = std::fs::read_to_string(path).ok();
            snap.insert(path.clone(), content);
        }
        self.checkpoints.push(snap);
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn restore_to_checkpoint(&self, idx: usize) -> anyhow::Result<Vec<String>> {
        let snap = self
            .checkpoints
            .get(idx)
            .ok_or_else(|| anyhow::anyhow!("no checkpoint at index {}", idx))?;
        let mut restored = Vec::new();
        for (path, content) in snap {
            match content {
                None => {
                    if path.exists() {
                        std::fs::remove_file(path)?;
                        restored.push(format!("deleted {}", path.display()));
                    }
                }
                Some(c) => {
                    std::fs::write(path, c)?;
                    restored.push(format!("restored {}", path.display()));
                }
            }
        }
        for (path, original) in &self.originals {
            if !snap.contains_key(path) {
                match original {
                    None => {
                        if path.exists() {
                            std::fs::remove_file(path)?;
                            restored.push(format!("deleted {}", path.display()));
                        }
                    }
                    Some(c) => {
                        std::fs::write(path, c)?;
                        restored.push(format!("restored {}", path.display()));
                    }
                }
            }
        }
        Ok(restored)
    }

    pub fn truncate_checkpoints(&mut self, count: usize) {
        self.checkpoints.truncate(count);
    }
}

pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
}

impl ChangeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Created => "+",
            Self::Modified => "~",
            Self::Deleted => "-",
        }
    }
}
