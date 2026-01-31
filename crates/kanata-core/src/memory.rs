//! Project memory — persists project-level context in `.kanata/memory.yaml`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Persistent project memory stored in `.kanata/memory.yaml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMemory {
    /// Key-value facts about the project.
    pub facts: HashMap<String, String>,
    /// File path where this memory is stored.
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl ProjectMemory {
    /// Loads project memory from a file, or returns an empty one if the file
    /// does not exist.
    pub fn load(path: &Path) -> Self {
        let mut memory = if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|content| serde_yaml::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        };
        memory.path = Some(path.to_path_buf());
        memory
    }

    /// Saves the memory to its file path.
    ///
    /// # Errors
    ///
    /// Returns an error if no path is set, or serialization/writing fails.
    pub fn save(&self) -> Result<(), String> {
        let path =
            self.path.as_ref().ok_or_else(|| "No path set for project memory".to_string())?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        let yaml =
            serde_yaml::to_string(self).map_err(|e| format!("Failed to serialize memory: {e}"))?;
        std::fs::write(path, yaml).map_err(|e| format!("Failed to write memory: {e}"))
    }

    /// Stores a fact.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.facts.insert(key.into(), value.into());
    }

    /// Retrieves a fact.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.facts.get(key).map(String::as_str)
    }

    /// Returns all facts as a formatted string for the system prompt.
    pub fn as_context_string(&self) -> String {
        if self.facts.is_empty() {
            return String::new();
        }
        let mut lines = vec!["## Project Memory".to_string()];
        for (k, v) in &self.facts {
            lines.push(format!("- **{k}**: {v}"));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_set_get() {
        let mut mem = ProjectMemory::default();
        mem.set("language", "Rust");
        assert_eq!(mem.get("language"), Some("Rust"));
        assert!(mem.get("missing").is_none());
    }

    #[test]
    fn test_memory_save_and_load() {
        let dir = std::env::temp_dir().join("kanata_test_memory");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.yaml");

        let mut mem = ProjectMemory::load(&path);
        mem.set("tech_stack", "Rust + tokio");
        mem.set("framework", "ratatui");
        mem.save().unwrap();

        let loaded = ProjectMemory::load(&path);
        assert_eq!(loaded.get("tech_stack"), Some("Rust + tokio"));
        assert_eq!(loaded.get("framework"), Some("ratatui"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_memory_context_string() {
        let mut mem = ProjectMemory::default();
        assert!(mem.as_context_string().is_empty());

        mem.set("lang", "Rust");
        let ctx = mem.as_context_string();
        assert!(ctx.contains("Project Memory"));
        assert!(ctx.contains("Rust"));
    }
}
