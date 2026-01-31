//! Undo mechanism — records file operations and supports rollback.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Maximum number of operations to keep in the undo stack.
const MAX_UNDO_STACK: usize = 50;

/// A recorded file operation that can be undone.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// The file that was modified.
    pub path: PathBuf,
    /// The previous content (before the operation). `None` if the file was
    /// newly created.
    pub previous_content: Option<String>,
    /// Description of the operation.
    pub description: String,
}

/// Manages the undo stack for file operations.
pub struct UndoManager {
    stack: VecDeque<UndoEntry>,
}

impl UndoManager {
    /// Creates a new empty undo manager.
    pub fn new() -> Self {
        Self { stack: VecDeque::new() }
    }

    /// Records a file's state before modification. Call this before writing.
    pub fn record_before_write(&mut self, path: &Path, description: impl Into<String>) {
        let previous_content = std::fs::read_to_string(path).ok();
        self.push(UndoEntry {
            path: path.to_path_buf(),
            previous_content,
            description: description.into(),
        });
    }

    /// Pushes an entry onto the undo stack.
    fn push(&mut self, entry: UndoEntry) {
        if self.stack.len() >= MAX_UNDO_STACK {
            self.stack.pop_front();
        }
        self.stack.push_back(entry);
    }

    /// Undoes the most recent operation. Returns a description of what was
    /// undone, or an error if the stack is empty or the rollback fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the undo stack is empty or the file operation fails.
    pub fn undo(&mut self) -> Result<String, String> {
        let entry = self.stack.pop_back().ok_or_else(|| "Nothing to undo".to_string())?;

        if let Some(content) = &entry.previous_content {
            std::fs::write(&entry.path, content)
                .map_err(|e| format!("Failed to restore {}: {e}", entry.path.display()))?;
            Ok(format!("Undone: {} (restored {})", entry.description, entry.path.display()))
        } else {
            // File was newly created — remove it.
            if entry.path.exists() {
                std::fs::remove_file(&entry.path)
                    .map_err(|e| format!("Failed to remove {}: {e}", entry.path.display()))?;
            }
            Ok(format!("Undone: {} (removed {})", entry.description, entry.path.display()))
        }
    }

    /// Returns the number of operations in the undo stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns true if the undo stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Peeks at the most recent undo entry without popping it.
    pub fn peek(&self) -> Option<&UndoEntry> {
        self.stack.back()
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_restores_previous_content() {
        let dir = std::env::temp_dir().join("kanata_test_undo");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        std::fs::write(&file, "original").unwrap();

        let mut mgr = UndoManager::new();
        mgr.record_before_write(&file, "Edit test.txt");

        // Simulate a write.
        std::fs::write(&file, "modified").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "modified");

        // Undo.
        let msg = mgr.undo().unwrap();
        assert!(msg.contains("Undone"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_undo_removes_new_file() {
        let dir = std::env::temp_dir().join("kanata_test_undo_new");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("new.txt");

        let mut mgr = UndoManager::new();
        mgr.record_before_write(&file, "Create new.txt");

        // Simulate creating a new file.
        std::fs::write(&file, "new content").unwrap();

        let msg = mgr.undo().unwrap();
        assert!(msg.contains("removed"));
        assert!(!file.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_undo_empty_stack() {
        let mut mgr = UndoManager::new();
        assert!(mgr.undo().is_err());
    }

    #[test]
    fn test_undo_stack_limit() {
        let mut mgr = UndoManager::new();
        for i in 0..60 {
            mgr.push(UndoEntry {
                path: PathBuf::from(format!("/tmp/file{i}.txt")),
                previous_content: Some("content".to_string()),
                description: format!("op {i}"),
            });
        }
        assert_eq!(mgr.len(), MAX_UNDO_STACK);
    }
}
