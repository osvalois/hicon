use serde::{Serialize, Deserialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub command: String,
}

pub struct Log {
    entries: Vec<LogEntry>,
    file_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("IO error: {0}")]
    IOError(#[from] io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Index out of bounds")]
    IndexOutOfBounds,
}

impl Log {
    /// Creates a new Log instance.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path where the log will be persisted.
    pub fn new(file_path: &str) -> Result<Self, LogError> {
        let mut log = Log {
            entries: Vec::new(),
            file_path: file_path.to_string(),
        };
        log.load_from_file()?;
        Ok(log)
    }

    /// Appends a new entry to the log.
    ///
    /// # Arguments
    ///
    /// * `term` - The term number for this entry.
    /// * `command` - The command to be logged.
    pub fn append(&mut self, term: u64, command: String) -> Result<u64, LogError> {
        let index = self.entries.len() as u64;
        let entry = LogEntry { index, term, command };
        self.entries.push(entry);
        self.persist_entry(&self.entries.last().unwrap())?;
        Ok(index)
    }

    /// Retrieves an entry at a specific index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the entry to retrieve.
    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        self.entries.get(index as usize)
    }

    /// Returns the last entry in the log.
    pub fn last(&self) -> Option<&LogEntry> {
        self.entries.last()
    }

    /// Returns the number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Truncates the log starting from a given index.
    ///
    /// # Arguments
    ///
    /// * `from_index` - The index from which to start truncation.
    pub fn truncate(&mut self, from_index: u64) -> Result<(), LogError> {
        if from_index as usize > self.entries.len() {
            return Err(LogError::IndexOutOfBounds);
        }
        self.entries.truncate(from_index as usize);
        self.persist_all()?;
        Ok(())
    }

    /// Persists a single entry to the log file.
    fn persist_entry(&self, entry: &LogEntry) -> Result<(), LogError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        let entry_json = serde_json::to_string(&entry)?;
        writeln!(file, "{}", entry_json)?;
        Ok(())
    }

    /// Persists all entries to the log file.
    fn persist_all(&self) -> Result<(), LogError> {
        let file = File::create(&self.file_path)?;
        serde_json::to_writer(file, &self.entries)?;
        Ok(())
    }

    /// Loads entries from the log file.
    fn load_from_file(&mut self) -> Result<(), LogError> {
        if !Path::new(&self.file_path).exists() {
            return Ok(());
        }

        let mut file = File::open(&self.file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        for line in contents.lines() {
            let entry: LogEntry = serde_json::from_str(line)?;
            self.entries.push(entry);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_log_operations() -> Result<(), LogError> {
        let temp_file = NamedTempFile::new()?;
        let file_path = temp_file.path().to_str().unwrap();

        let mut log = Log::new(file_path)?;

        // Test append
        log.append(1, "command1".to_string())?;
        log.append(1, "command2".to_string())?;
        log.append(2, "command3".to_string())?;

        assert_eq!(log.len(), 3);

        // Test get
        let entry = log.get(1).unwrap();
        assert_eq!(entry.command, "command2");

        // Test last
        let last = log.last().unwrap();
        assert_eq!(last.command, "command3");

        // Test truncate
        log.truncate(2)?;
        assert_eq!(log.len(), 2);

        // Test persistence
        drop(log);
        let new_log = Log::new(file_path)?;
        assert_eq!(new_log.len(), 2);
        assert_eq!(new_log.last().unwrap().command, "command2");

        Ok(())
    }
}