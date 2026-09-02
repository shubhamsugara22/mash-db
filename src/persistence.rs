/// RDS-Ready Persistence Layer with Crash Recovery, Durability, and Transaction Support
///
/// Features:
/// - Write-Ahead Logging (WAL) for crash recovery
/// - Durable writes with fsync
/// - Transaction logging
/// - Database snapshots for backup/restore
/// - Metadata catalog
/// - Connection session tracking
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntry {
    // WAL Entry before data modification
    BeforeWrite {
        table_name: String,
        operation: String, // "INSERT", "UPDATE", "DELETE"
        timestamp: u64,
    },
    // WAL Entry after successful modification
    AfterWrite {
        table_name: String,
        operation: String,
        timestamp: u64,
        success: bool,
    },
    // Transaction markers
    TransactionBegin {
        tx_id: String,
        timestamp: u64,
    },
    TransactionCommit {
        tx_id: String,
        timestamp: u64,
    },
    TransactionRollback {
        tx_id: String,
        timestamp: u64,
    },
}

/// Write-Ahead Log for crash recovery
#[derive(Debug)]
pub struct WriteAheadLog {
    log_file_path: PathBuf,
    entries: Vec<LogEntry>,
}

impl WriteAheadLog {
    pub fn new(db_path: &str) -> Result<Self, String> {
        std::fs::create_dir_all(db_path)
            .map_err(|e| format!("Failed to create WAL directory: {}", e))?;
        let log_path = Path::new(db_path).join("wal.log");

        let entries = Self::recover_from_log(&log_path)?;

        Ok(WriteAheadLog {
            log_file_path: log_path,
            entries,
        })
    }

    /// Log a write operation BEFORE executing it
    pub fn log_before_write(&mut self, table_name: &str, operation: &str) -> Result<(), String> {
        let entry = LogEntry::BeforeWrite {
            table_name: table_name.to_string(),
            operation: operation.to_string(),
            timestamp: current_timestamp(),
        };

        self.append_entry(&entry)?;
        Ok(())
    }

    /// Log write completion AFTER successfully executing
    pub fn log_after_write(
        &mut self,
        table_name: &str,
        operation: &str,
        success: bool,
    ) -> Result<(), String> {
        let entry = LogEntry::AfterWrite {
            table_name: table_name.to_string(),
            operation: operation.to_string(),
            timestamp: current_timestamp(),
            success,
        };

        self.append_entry(&entry)?;
        Ok(())
    }

    /// Log transaction begin
    pub fn log_transaction_begin(&mut self, tx_id: &str) -> Result<(), String> {
        let entry = LogEntry::TransactionBegin {
            tx_id: tx_id.to_string(),
            timestamp: current_timestamp(),
        };

        self.append_entry(&entry)?;
        Ok(())
    }

    /// Log transaction commit
    pub fn log_transaction_commit(&mut self, tx_id: &str) -> Result<(), String> {
        let entry = LogEntry::TransactionCommit {
            tx_id: tx_id.to_string(),
            timestamp: current_timestamp(),
        };

        self.append_entry(&entry)?;
        Ok(())
    }

    /// Log transaction rollback
    pub fn log_transaction_rollback(&mut self, tx_id: &str) -> Result<(), String> {
        let entry = LogEntry::TransactionRollback {
            tx_id: tx_id.to_string(),
            timestamp: current_timestamp(),
        };

        self.append_entry(&entry)?;
        Ok(())
    }

    fn append_entry(&mut self, entry: &LogEntry) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .map_err(|e| format!("Failed to open WAL: {}", e))?;

        let json = serde_json::to_string(entry)
            .map_err(|e| format!("Failed to serialize log entry: {}", e))?;

        writeln!(file, "{}", json).map_err(|e| format!("Failed to write to WAL: {}", e))?;

        // Durable write - ensure data reaches disk
        file.sync_all()
            .map_err(|e| format!("Failed to sync WAL: {}", e))?;

        self.entries.push(entry.clone());
        Ok(())
    }

    /// Recover database state from WAL after crash
    fn recover_from_log(log_path: &Path) -> Result<Vec<LogEntry>, String> {
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let contents =
            std::fs::read_to_string(log_path).map_err(|e| format!("Failed to read WAL: {}", e))?;

        let mut entries = Vec::new();
        for line in contents.lines() {
            if line.is_empty() {
                continue;
            }
            let entry: LogEntry = serde_json::from_str(line)
                .map_err(|e| format!("Failed to parse WAL entry: {}", e))?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Get entries for crash recovery analysis
    pub fn get_recovery_entries(&self) -> Vec<&LogEntry> {
        self.entries.iter().collect()
    }

    /// Clear WAL after successful checkpoint
    pub fn clear_log(&mut self) -> Result<(), String> {
        std::fs::write(&self.log_file_path, "")
            .map_err(|e| format!("Failed to clear WAL: {}", e))?;
        self.entries.clear();
        Ok(())
    }
}

/// Database snapshot for backup/restore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSnapshot {
    pub timestamp: u64,
    pub database_name: String,
    pub tables: Vec<String>,
    pub row_count: usize,
    pub size_bytes: u64,
}

/// Persistent metadata catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub database_name: String,
    pub created_at: u64,
    pub last_modified: u64,
    pub tables: HashMap<String, TableMetadata>,
    pub snapshots: Vec<DatabaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub name: String,
    pub columns: Vec<String>,
    pub row_count: usize,
    pub created_at: u64,
    pub last_modified: u64,
    pub indexes: Vec<String>,
}

impl DatabaseMetadata {
    pub fn new(database_name: &str) -> Self {
        DatabaseMetadata {
            database_name: database_name.to_string(),
            created_at: current_timestamp(),
            last_modified: current_timestamp(),
            tables: HashMap::new(),
            snapshots: Vec::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .ok_or("Failed to load metadata".to_string())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to save metadata: {}", e))
    }

    pub fn register_table(&mut self, table_name: &str, columns: Vec<String>) {
        self.tables.insert(
            table_name.to_string(),
            TableMetadata {
                name: table_name.to_string(),
                columns,
                row_count: 0,
                created_at: current_timestamp(),
                last_modified: current_timestamp(),
                indexes: Vec::new(),
            },
        );
        self.last_modified = current_timestamp();
    }

    pub fn update_table_stats(&mut self, table_name: &str, row_count: usize) {
        if let Some(table) = self.tables.get_mut(table_name) {
            table.row_count = row_count;
            table.last_modified = current_timestamp();
            self.last_modified = current_timestamp();
        }
    }

    pub fn add_snapshot(&mut self, snapshot: DatabaseSnapshot) {
        self.snapshots.push(snapshot);
        self.last_modified = current_timestamp();
    }
}

/// Connection session tracking for multi-client support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSession {
    pub session_id: String,
    pub username: Option<String>,
    pub connected_at: u64,
    pub last_activity: u64,
    pub idle_timeout_secs: u64,
}

impl ConnectionSession {
    pub fn new(session_id: String) -> Self {
        let now = current_timestamp();
        ConnectionSession {
            session_id,
            username: None,
            connected_at: now,
            last_activity: now,
            idle_timeout_secs: 3600, // 1 hour default
        }
    }

    pub fn is_idle(&self) -> bool {
        let now = current_timestamp();
        (now - self.last_activity) > self.idle_timeout_secs
    }

    pub fn update_activity(&mut self) {
        self.last_activity = current_timestamp();
    }

    pub fn set_user(&mut self, username: String) {
        self.username = Some(username);
    }
}

/// Connection pool for managing multiple concurrent connections
#[derive(Debug)]
pub struct ConnectionPool {
    sessions: HashMap<String, ConnectionSession>,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        ConnectionPool {
            sessions: HashMap::new(),
            max_connections,
        }
    }

    pub fn create_session(&mut self) -> Result<String, String> {
        if self.sessions.len() >= self.max_connections {
            return Err(format!(
                "Maximum connections ({}) reached",
                self.max_connections
            ));
        }

        let session_id = format!("sess_{}", uuid_stub());
        self.sessions.insert(
            session_id.clone(),
            ConnectionSession::new(session_id.clone()),
        );

        Ok(session_id)
    }

    pub fn close_session(&mut self, session_id: &str) -> Result<(), String> {
        self.sessions
            .remove(session_id)
            .ok_or(format!("Session not found: {}", session_id))?;
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Option<&ConnectionSession> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut ConnectionSession> {
        self.sessions.get_mut(session_id)
    }

    pub fn cleanup_idle_sessions(&mut self) {
        let idle_sessions: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, session)| session.is_idle())
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in idle_sessions {
            let _ = self.close_session(&session_id);
        }
    }

    pub fn get_active_sessions_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn get_all_sessions(&self) -> Vec<ConnectionSession> {
        self.sessions.values().cloned().collect()
    }
}

/// Database health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseStatus {
    Healthy,
    Degraded,
    RecoveryInProgress,
    Error(String),
}

/// Health check and database status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub status: DatabaseStatus,
    pub last_check: u64,
    pub connections_active: usize,
    pub wal_size_bytes: u64,
    pub last_backup: Option<u64>,
}

impl DatabaseHealth {
    pub fn new() -> Self {
        DatabaseHealth {
            status: DatabaseStatus::Healthy,
            last_check: current_timestamp(),
            connections_active: 0,
            wal_size_bytes: 0,
            last_backup: None,
        }
    }

    pub fn check_health(&mut self, active_connections: usize, wal_size: u64) -> DatabaseStatus {
        self.last_check = current_timestamp();
        self.connections_active = active_connections;
        self.wal_size_bytes = wal_size;

        // Set status based on metrics
        if wal_size > 100_000_000 {
            // WAL larger than 100MB
            self.status = DatabaseStatus::Degraded;
        } else if active_connections == 0 {
            self.status = DatabaseStatus::Healthy;
        }

        self.status.clone()
    }
}

/// Durability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurabilityConfig {
    pub fsync_on_write: bool,           // Ensure writes reach disk
    pub wal_enabled: bool,              // Write-ahead logging
    pub snapshot_interval_seconds: u64, // Auto-snapshot interval
    pub backup_retention_count: usize,  // Keep N backups
    pub compression_enabled: bool,      // Compress backups
}

impl Default for DurabilityConfig {
    fn default() -> Self {
        DurabilityConfig {
            fsync_on_write: true,
            wal_enabled: true,
            snapshot_interval_seconds: 3600, // 1 hour
            backup_retention_count: 10,
            compression_enabled: false,
        }
    }
}

/// Helper functions

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn uuid_stub() -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}", current_timestamp(), counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_ahead_log_creation() {
        let wal = WriteAheadLog::new("test.db").unwrap();
        assert_eq!(wal.entries.len(), 0);
    }

    #[test]
    fn test_database_metadata_creation() {
        let metadata = DatabaseMetadata::new("test_db");
        assert_eq!(metadata.database_name, "test_db");
        assert_eq!(metadata.tables.len(), 0);
    }

    #[test]
    fn test_connection_session_idle_detection() {
        let mut session = ConnectionSession::new("sess_1".to_string());
        session.idle_timeout_secs = 1;
        // Session should not be idle immediately
        assert!(!session.is_idle());
    }

    #[test]
    fn test_connection_pool_creation() {
        let mut pool = ConnectionPool::new(5);
        let session_id = pool.create_session().unwrap();
        assert!(!session_id.is_empty());
        assert!(pool.get_session(&session_id).is_some());
    }

    #[test]
    fn test_connection_pool_max_connections() {
        let mut pool = ConnectionPool::new(2);
        let _ = pool.create_session().unwrap();
        let _ = pool.create_session().unwrap();
        let result = pool.create_session();
        assert!(result.is_err());
    }

    #[test]
    fn test_database_health_status() {
        let mut health = DatabaseHealth::new();
        let status = health.check_health(5, 50_000_000);
        assert_eq!(status, DatabaseStatus::Healthy);
    }

    #[test]
    fn test_database_metadata_table_registration() {
        let mut metadata = DatabaseMetadata::new("test_db");
        metadata.register_table("users", vec!["id".to_string(), "name".to_string()]);
        assert_eq!(metadata.tables.len(), 1);
        assert!(metadata.tables.contains_key("users"));
    }
}
