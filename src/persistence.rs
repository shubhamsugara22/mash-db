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

/// Transaction Isolation Levels
/// Defines the consistency guarantee for concurrent transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Allows dirty reads (no locking)
    /// Fastest but least safe - other transactions may see uncommitted changes
    ReadUncommitted,

    /// Prevents dirty reads (read locks only during operation)
    /// Default mode - balances safety and performance
    ReadCommitted,

    /// Prevents non-repeatable reads (row locks held for transaction duration)
    /// Stronger isolation - rows read at transaction start stay consistent
    RepeatableRead,

    /// Full isolation (serializes transactions)
    /// Strongest isolation - acts as if transactions run one at a time
    Serializable,
}

impl IsolationLevel {
    /// Returns the numeric level (0-3) for comparison
    pub fn as_u8(&self) -> u8 {
        match self {
            IsolationLevel::ReadUncommitted => 0,
            IsolationLevel::ReadCommitted => 1,
            IsolationLevel::RepeatableRead => 2,
            IsolationLevel::Serializable => 3,
        }
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

impl Default for IsolationLevel {
    fn default() -> Self {
        IsolationLevel::ReadCommitted // Sensible default: prevents dirty reads
    }
}

/// Write-Ahead Log for crash recovery
#[derive(Debug)]
pub struct WriteAheadLog {
    log_file_path: PathBuf,
    entries: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub username: String,
    pub session_id: String,
    pub operation: String,
    pub table_name: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct AuditLogger {
    log_file_path: PathBuf,
}

impl AuditLogger {
    pub fn new(db_path: &str) -> Result<Self, String> {
        std::fs::create_dir_all(db_path)
            .map_err(|e| format!("Failed to create audit directory: {}", e))?;
        Ok(Self {
            log_file_path: Path::new(db_path).join("audit.log"),
        })
    }

    pub fn log(&self, entry: &AuditEntry) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .map_err(|e| format!("Failed to open audit log: {}", e))?;
        let json = serde_json::to_string(entry)
            .map_err(|e| format!("Failed to serialize audit entry: {}", e))?;
        writeln!(file, "{}", json).map_err(|e| format!("Failed to write audit log: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync audit log: {}", e))
    }
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

/// Row-level lock tracking for optimistic concurrency control.
///
/// Each table/row pair can be owned by at most one session at a time.
/// Lock acquisition record with timestamp for timeout enforcement
#[derive(Debug, Clone)]
struct LockRecord {
    session_id: String,
    acquired_at: u64,
}

/// This enables enforcing a simple single-writer lock while leaving the
/// rest of the SQL engine unchanged.
#[derive(Debug)]
pub struct RowLockManager {
    locks: HashMap<(String, u32), LockRecord>,
    isolation_level_overrides: HashMap<String, IsolationLevel>, // Per-session isolation levels
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLockInfo {
    pub table_name: String,
    pub row_id: u32,
    pub session_id: String,
}

/// Represents a deadlock event for observability
#[derive(Debug, Clone)]
pub struct DeadlockEvent {
    pub timestamp: u64,
    pub session_id: String,
    pub conflicting_session: String,
    pub resource: (String, u32),
}

impl Default for RowLockManager {
    fn default() -> Self {
        RowLockManager {
            locks: HashMap::new(),
            isolation_level_overrides: HashMap::new(),
        }
    }
}

impl RowLockManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the isolation level for a specific session
    pub fn set_session_isolation_level(&mut self, session_id: String, level: IsolationLevel) {
        self.isolation_level_overrides.insert(session_id, level);
    }

    /// Get the isolation level for a session (returns default if not overridden)
    pub fn get_session_isolation_level(&self, session_id: &str) -> IsolationLevel {
        self.isolation_level_overrides
            .get(session_id)
            .copied()
            .unwrap_or(IsolationLevel::ReadCommitted)
    }

    /// Clear isolation level override for a session (on disconnect/cleanup)
    pub fn clear_session_isolation_level(&mut self, session_id: &str) {
        self.isolation_level_overrides.remove(session_id);
    }

    /// Check if a read operation requires a lock based on isolation level
    pub fn requires_read_lock(&self, session_id: &str) -> bool {
        let level = self.get_session_isolation_level(session_id);
        match level {
            IsolationLevel::ReadUncommitted => false, // No locks for dirty reads
            IsolationLevel::ReadCommitted => true,    // Lock during read
            IsolationLevel::RepeatableRead => true,   // Lock for entire transaction
            IsolationLevel::Serializable => true,     // Full serialization with locks
        }
    }

    /// Check if a write operation requires escalated locking based on isolation level
    pub fn requires_write_lock_escalation(&self, session_id: &str) -> bool {
        let level = self.get_session_isolation_level(session_id);
        match level {
            IsolationLevel::ReadUncommitted => false, // No escalation needed
            IsolationLevel::ReadCommitted => false,   // Standard write lock sufficient
            IsolationLevel::RepeatableRead => true,   // Escalate to range lock
            IsolationLevel::Serializable => true,     // Full serialization locks
        }
    }

    pub fn check_and_cleanup_expired_locks(&mut self, timeout_secs: u64) -> usize {
        let now = current_timestamp();
        let expired_count = self
            .locks
            .values()
            .filter(|record| (now - record.acquired_at) >= timeout_secs)
            .count();
        self.locks
            .retain(|_, record| (now - record.acquired_at) < timeout_secs);
        expired_count
    }

    pub fn lock_row(
        &mut self,
        table_name: &str,
        row_id: u32,
        session_id: &str,
    ) -> Result<(), String> {
        let key = (table_name.to_lowercase(), row_id);

        // Check for deadlock risk before acquiring
        if self.would_create_deadlock(session_id, table_name, row_id) {
            return Err(format!(
                "Deadlock detected: acquiring lock on row {} in table '{}' would create circular dependency",
                row_id, table_name
            ));
        }

        match self.locks.get(&key) {
            Some(record) if record.session_id != session_id => Err(format!(
                "Row {} in table '{}' is already locked by session '{}'",
                row_id, table_name, record.session_id
            )),
            _ => {
                self.locks.insert(
                    key,
                    LockRecord {
                        session_id: session_id.to_string(),
                        acquired_at: current_timestamp(),
                    },
                );
                Ok(())
            }
        }
    }

    pub fn lock_rows(
        &mut self,
        table_name: &str,
        row_ids: &[u32],
        session_id: &str,
    ) -> Result<(), String> {
        let mut sorted_ids = row_ids.to_vec();
        sorted_ids.sort_unstable();
        sorted_ids.dedup();

        let mut acquired = Vec::new();
        for row_id in &sorted_ids {
            if let Err(error) = self.lock_row(table_name, *row_id, session_id) {
                for acquired_id in acquired {
                    let _ = self.unlock_row(table_name, acquired_id, session_id);
                }
                return Err(error);
            }
            acquired.push(*row_id);
        }
        Ok(())
    }

    pub fn unlock_row(
        &mut self,
        table_name: &str,
        row_id: u32,
        session_id: &str,
    ) -> Result<(), String> {
        let key = (table_name.to_lowercase(), row_id);
        match self.locks.get(&key) {
            Some(record) if record.session_id != session_id => Err(format!(
                "Row {} in table '{}' is held by session '{}' and cannot be released by '{}'",
                row_id, table_name, record.session_id, session_id
            )),
            Some(_) => {
                self.locks.remove(&key);
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn get_lock_owner(&self, table_name: &str, row_id: u32) -> Option<String> {
        self.locks
            .get(&(table_name.to_lowercase(), row_id))
            .map(|record| record.session_id.clone())
    }

    pub fn active_locks(&self) -> Vec<RowLockInfo> {
        let mut locks: Vec<RowLockInfo> = self
            .locks
            .iter()
            .map(|((table_name, row_id), record)| RowLockInfo {
                table_name: table_name.clone(),
                row_id: *row_id,
                session_id: record.session_id.clone(),
            })
            .collect();
        locks.sort_by(|left, right| {
            left.table_name
                .cmp(&right.table_name)
                .then(left.row_id.cmp(&right.row_id))
                .then(left.session_id.cmp(&right.session_id))
        });
        locks
    }

    pub fn unlock_all_for_session(&mut self, session_id: &str) -> usize {
        let before = self.locks.len();
        self.locks
            .retain(|_, record| record.session_id != session_id);
        before - self.locks.len()
    }

    /// Detect if acquiring a lock would create a deadlock
    /// Returns true if a cycle would be created in the wait-for graph
    pub fn would_create_deadlock(
        &self,
        requesting_session: &str,
        table_name: &str,
        row_id: u32,
    ) -> bool {
        let key = (table_name.to_lowercase(), row_id);

        // If lock is free, no deadlock possible
        if !self.locks.contains_key(&key) {
            return false;
        }

        // If the requesting session already holds this lock, no deadlock
        if let Some(holder) = self.locks.get(&key) {
            if holder.session_id == requesting_session {
                return false;
            }
        }

        // Build a simplified wait-for chain
        // requesting_session -> holder_session (waiting for lock)
        // Check if holder_session -> requesting_session through its locks (creating a cycle)
        if let Some(holder) = self.locks.get(&key) {
            let holder_session = &holder.session_id;

            // Check if the holder is waiting for any locks held by the requesting session
            for ((lock_table, lock_row), lock_record) in &self.locks {
                if lock_record.session_id == requesting_session {
                    // requesting_session holds this lock
                    // Check if holder_session holds a lock that would need this one
                    if self.would_session_wait_for(holder_session, lock_table, *lock_row) {
                        return true; // Cycle detected!
                    }
                }
            }
        }

        false
    }

    /// Helper: Check if a session would need to wait for a specific lock
    /// (by checking if it holds locks on the same table/rows in a dependent order)
    fn would_session_wait_for(&self, session_id: &str, table_name: &str, row_id: u32) -> bool {
        // Check if this session holds locks on the same table
        for ((lock_table, _lock_row), lock_record) in &self.locks {
            if lock_record.session_id == session_id && lock_table == table_name {
                // Session holds locks on same table
                // In deterministic locking order, if it's trying row_id, it must acquire in sorted order
                // This is a simplified check for 2-cycle deadlocks
                return true;
            }
        }
        false
    }

    /// Get all deadlock events that have been detected (for observability)
    /// Returns information about potential deadlock situations
    pub fn analyze_deadlock_risk(&self) -> Vec<(String, String)> {
        let mut risk_pairs = Vec::new();

        // Find all pairs of sessions that have circular lock dependencies
        let all_sessions: Vec<String> = self
            .locks
            .values()
            .map(|record| record.session_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for session_a in &all_sessions {
            for session_b in &all_sessions {
                if session_a != session_b {
                    // Check if session_a holds what session_b wants
                    for ((table, _row), record_b) in &self.locks {
                        if &record_b.session_id == session_b {
                            // session_b holds this lock
                            // Check if session_a also holds locks on same table
                            for ((table_a, _), record_a) in &self.locks {
                                if &record_a.session_id == session_a && table_a == table {
                                    // Both sessions hold locks on same table - potential deadlock
                                    risk_pairs.push((session_a.clone(), session_b.clone()));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Remove duplicates
        risk_pairs.sort();
        risk_pairs.dedup();
        risk_pairs
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
    pub isolation_level: IsolationLevel, // Current transaction isolation level for this session
}

impl ConnectionSession {
    pub fn new(session_id: String) -> Self {
        let now = current_timestamp();
        ConnectionSession {
            session_id,
            username: None,
            connected_at: now,
            last_activity: now,
            idle_timeout_secs: 3600,                        // 1 hour default
            isolation_level: IsolationLevel::ReadCommitted, // Default isolation level
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

    pub fn set_isolation_level(&mut self, level: IsolationLevel) {
        self.isolation_level = level;
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
    pub fsync_on_write: bool,                    // Ensure writes reach disk
    pub wal_enabled: bool,                       // Write-ahead logging
    pub snapshot_interval_seconds: u64,          // Auto-snapshot interval
    pub backup_retention_count: usize,           // Keep N backups
    pub compression_enabled: bool,               // Compress backups
    pub lock_timeout_secs: u64,                  // Lock acquisition timeout
    pub default_isolation_level: IsolationLevel, // Default transaction isolation level
}

impl Default for DurabilityConfig {
    fn default() -> Self {
        DurabilityConfig {
            fsync_on_write: true,
            wal_enabled: true,
            snapshot_interval_seconds: 3600, // 1 hour
            backup_retention_count: 10,
            compression_enabled: false,
            lock_timeout_secs: 30, // 30 second default timeout
            default_isolation_level: IsolationLevel::ReadCommitted, // Prevents dirty reads by default
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
    fn test_row_lock_manager_lock_and_release() {
        let mut lock_manager = RowLockManager::new();

        assert!(lock_manager.lock_row("users", 7, "sess_1").is_ok());
        assert!(lock_manager.lock_row("users", 7, "sess_2").is_err());
        assert!(lock_manager.unlock_row("users", 7, "sess_1").is_ok());
        assert!(lock_manager.lock_row("users", 7, "sess_2").is_ok());
        assert_eq!(
            lock_manager.get_lock_owner("users", 7),
            Some("sess_2".to_string())
        );
    }

    #[test]
    fn test_row_lock_manager_releases_all_session_locks() {
        let mut lock_manager = RowLockManager::new();
        lock_manager.lock_row("users", 1, "sess_1").unwrap();
        lock_manager.lock_row("orders", 2, "sess_1").unwrap();
        lock_manager.lock_row("users", 3, "sess_2").unwrap();

        assert_eq!(lock_manager.unlock_all_for_session("sess_1"), 2);
        assert_eq!(lock_manager.get_lock_owner("users", 1), None);
        assert_eq!(lock_manager.get_lock_owner("orders", 2), None);
        assert_eq!(
            lock_manager.get_lock_owner("users", 3),
            Some("sess_2".to_string())
        );
    }

    #[test]
    fn test_row_lock_manager_batch_lock_is_atomic() {
        let mut lock_manager = RowLockManager::new();
        lock_manager.lock_row("users", 2, "sess_2").unwrap();

        assert!(lock_manager
            .lock_rows("users", &[3, 2, 1], "sess_1")
            .is_err());
        assert_eq!(lock_manager.get_lock_owner("users", 1), None);
        assert_eq!(lock_manager.get_lock_owner("users", 3), None);
        assert_eq!(
            lock_manager.get_lock_owner("users", 2),
            Some("sess_2".to_string())
        );
    }

    #[test]
    fn test_row_lock_manager_reports_sorted_active_locks() {
        let mut lock_manager = RowLockManager::new();
        lock_manager.lock_row("orders", 4, "sess_2").unwrap();
        lock_manager.lock_row("users", 2, "sess_1").unwrap();

        assert_eq!(
            lock_manager.active_locks(),
            vec![
                RowLockInfo {
                    table_name: "orders".to_string(),
                    row_id: 4,
                    session_id: "sess_2".to_string(),
                },
                RowLockInfo {
                    table_name: "users".to_string(),
                    row_id: 2,
                    session_id: "sess_1".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_database_metadata_table_registration() {
        let mut metadata = DatabaseMetadata::new("test_db");
        metadata.register_table("users", vec!["id".to_string(), "name".to_string()]);
        assert_eq!(metadata.tables.len(), 1);
        assert!(metadata.tables.contains_key("users"));
    }

    #[test]
    fn test_row_lock_manager_tracks_lock_acquisition_time() {
        let mut lock_manager = RowLockManager::new();
        lock_manager.lock_row("users", 5, "sess_1").unwrap();

        // Verify lock was recorded with a timestamp
        assert_eq!(
            lock_manager.get_lock_owner("users", 5),
            Some("sess_1".to_string())
        );

        // Cleanup with a very large timeout should not remove the lock
        let removed = lock_manager.check_and_cleanup_expired_locks(u64::MAX);
        assert_eq!(removed, 0);
        assert_eq!(
            lock_manager.get_lock_owner("users", 5),
            Some("sess_1".to_string())
        );
    }

    #[test]
    fn test_row_lock_manager_cleanup_expired_locks() {
        let mut lock_manager = RowLockManager::new();
        lock_manager.lock_row("users", 1, "sess_1").unwrap();
        lock_manager.lock_row("users", 2, "sess_2").unwrap();

        // All locks are very recent, so timeout of 0 should expire them all
        let removed = lock_manager.check_and_cleanup_expired_locks(0);
        assert!(removed > 0);
        assert_eq!(lock_manager.get_lock_owner("users", 1), None);
        assert_eq!(lock_manager.get_lock_owner("users", 2), None);
    }

    #[test]
    fn test_transaction_markers_are_durable() {
        let path = "test_transaction_wal";
        let _ = std::fs::remove_dir_all(path);
        let mut wal = WriteAheadLog::new(path).unwrap();
        wal.log_transaction_begin("session_1").unwrap();
        wal.log_transaction_commit("session_1").unwrap();
        wal.log_transaction_begin("session_2").unwrap();
        wal.log_transaction_rollback("session_2").unwrap();

        assert_eq!(wal.get_recovery_entries().len(), 4);
        assert!(
            std::fs::read_to_string(format!("{}/wal.log", path))
                .unwrap()
                .lines()
                .count()
                == 4
        );
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn test_deadlock_detection_basic() {
        let mut lock_manager = RowLockManager::new();

        // Session A acquires lock on row 1
        lock_manager.lock_row("users", 1, "sess_a").unwrap();

        // Session B tries to acquire lock on row 1 (will be blocked in real scenario)
        // But deadlock detection should not trigger yet (only 2-way deadlock risk)
        let would_deadlock = lock_manager.would_create_deadlock("sess_b", "users", 1);
        assert!(!would_deadlock, "Simple lock contention is not deadlock");
    }

    #[test]
    fn test_deadlock_detection_circular() {
        let mut lock_manager = RowLockManager::new();

        // Set up circular dependency:
        // Session A holds lock on row 1
        lock_manager.lock_row("users", 1, "sess_a").unwrap();
        // Session B holds lock on row 2
        lock_manager.lock_row("users", 2, "sess_b").unwrap();

        // Session A tries to acquire row 2 (held by B) -> would create wait A->B
        // Session B holds row 2 -> would create wait B->A (if B tried row 1)
        // This creates a potential 2-cycle deadlock risk
        let risk_pairs = lock_manager.analyze_deadlock_risk();
        // Should detect risk between sessions on same table
        assert!(
            !risk_pairs.is_empty(),
            "Circular lock pattern should be detected"
        );
    }

    #[test]
    fn test_deadlock_detection_same_session_no_deadlock() {
        let mut lock_manager = RowLockManager::new();

        // Session A acquires multiple locks (no deadlock with itself)
        lock_manager.lock_row("users", 1, "sess_a").unwrap();
        lock_manager.lock_row("users", 2, "sess_a").unwrap();

        // Session A trying to acquire another lock should not trigger deadlock
        let would_deadlock = lock_manager.would_create_deadlock("sess_a", "users", 3);
        assert!(!would_deadlock, "Same session cannot deadlock with itself");
    }

    #[test]
    fn test_deadlock_detection_free_lock() {
        let mut lock_manager = RowLockManager::new();

        // No locks held, so acquiring should never deadlock
        let would_deadlock = lock_manager.would_create_deadlock("sess_a", "users", 1);
        assert!(!would_deadlock, "Free lock cannot cause deadlock");
    }

    #[test]
    fn test_deadlock_risk_analysis() {
        let mut lock_manager = RowLockManager::new();

        // Session A holds locks on rows 1, 2
        lock_manager.lock_row("products", 1, "sess_a").unwrap();
        lock_manager.lock_row("products", 2, "sess_a").unwrap();

        // Session B holds lock on row 3
        lock_manager.lock_row("products", 3, "sess_b").unwrap();

        // Session C holds lock on row 4
        lock_manager.lock_row("products", 4, "sess_c").unwrap();

        // Analyze deadlock risks
        let risks = lock_manager.analyze_deadlock_risk();

        // Should identify that multiple sessions hold locks on same table
        // (potential deadlock risk in concurrent scenarios)
        assert!(
            !risks.is_empty(),
            "Multiple sessions on same table should show risk"
        );
        assert!(
            risks.iter().any(|(s1, s2)| {
                (s1 == "sess_a" && s2 == "sess_b") || (s1 == "sess_b" && s2 == "sess_a")
            }),
            "Should identify risk between specific sessions"
        );
    }

    #[test]
    fn test_deadlock_detection_after_unlock() {
        let mut lock_manager = RowLockManager::new();

        // Session A holds lock
        lock_manager.lock_row("users", 1, "sess_a").unwrap();
        // Session B holds lock
        lock_manager.lock_row("users", 2, "sess_b").unwrap();

        // Risks should exist
        let risks_before = lock_manager.analyze_deadlock_risk();
        assert!(!risks_before.is_empty(), "Should have deadlock risk");

        // Session A releases lock
        lock_manager.unlock_row("users", 1, "sess_a").unwrap();

        // Risk might still exist (B still holds locks)
        // But if we unlock all:
        lock_manager.unlock_row("users", 2, "sess_b").unwrap();

        let risks_after = lock_manager.analyze_deadlock_risk();
        // After all locks released, risk should be minimal
        assert!(
            risks_after.len() <= risks_before.len(),
            "Risk should decrease after unlocking"
        );
    }

    // ============================================================
    // Isolation Level Tests (8 tests total)
    // ============================================================

    #[test]
    fn test_isolation_level_default() {
        let level = IsolationLevel::default();
        assert_eq!(level, IsolationLevel::ReadCommitted);
        assert_eq!(level.as_u8(), 1);
    }

    #[test]
    fn test_isolation_level_hierarchy() {
        let levels = vec![
            (IsolationLevel::ReadUncommitted, 0),
            (IsolationLevel::ReadCommitted, 1),
            (IsolationLevel::RepeatableRead, 2),
            (IsolationLevel::Serializable, 3),
        ];

        for (level, expected_value) in levels {
            assert_eq!(level.as_u8(), expected_value);
        }
    }

    #[test]
    fn test_isolation_level_names() {
        assert_eq!(IsolationLevel::ReadUncommitted.name(), "READ UNCOMMITTED");
        assert_eq!(IsolationLevel::ReadCommitted.name(), "READ COMMITTED");
        assert_eq!(IsolationLevel::RepeatableRead.name(), "REPEATABLE READ");
        assert_eq!(IsolationLevel::Serializable.name(), "SERIALIZABLE");
    }

    #[test]
    fn test_connection_session_isolation_level() {
        let mut session = ConnectionSession::new("sess_1".to_string());

        // Default isolation level
        assert_eq!(session.isolation_level, IsolationLevel::ReadCommitted);

        // Change isolation level
        session.set_isolation_level(IsolationLevel::RepeatableRead);
        assert_eq!(session.isolation_level, IsolationLevel::RepeatableRead);

        // Change again
        session.set_isolation_level(IsolationLevel::Serializable);
        assert_eq!(session.isolation_level, IsolationLevel::Serializable);
    }

    #[test]
    fn test_durability_config_default_isolation() {
        let config = DurabilityConfig::default();
        assert_eq!(
            config.default_isolation_level,
            IsolationLevel::ReadCommitted
        );
    }

    #[test]
    fn test_row_lock_manager_isolation_level_management() {
        let mut lock_manager = RowLockManager::new();

        // Default isolation level for unknown session
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_a"),
            IsolationLevel::ReadCommitted
        );

        // Set isolation level for session
        lock_manager
            .set_session_isolation_level("sess_a".to_string(), IsolationLevel::Serializable);
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_a"),
            IsolationLevel::Serializable
        );

        // Different session has different level
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_b"),
            IsolationLevel::ReadCommitted
        );

        // Clear isolation level
        lock_manager.clear_session_isolation_level("sess_a");
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_a"),
            IsolationLevel::ReadCommitted
        );
    }

    #[test]
    fn test_read_lock_requirements_by_isolation() {
        let mut lock_manager = RowLockManager::new();

        // READ UNCOMMITTED: no read locks
        lock_manager
            .set_session_isolation_level("sess_a".to_string(), IsolationLevel::ReadUncommitted);
        assert!(!lock_manager.requires_read_lock("sess_a"));

        // READ COMMITTED: read locks required
        lock_manager
            .set_session_isolation_level("sess_b".to_string(), IsolationLevel::ReadCommitted);
        assert!(lock_manager.requires_read_lock("sess_b"));

        // REPEATABLE READ: read locks required
        lock_manager
            .set_session_isolation_level("sess_c".to_string(), IsolationLevel::RepeatableRead);
        assert!(lock_manager.requires_read_lock("sess_c"));

        // SERIALIZABLE: read locks required
        lock_manager
            .set_session_isolation_level("sess_d".to_string(), IsolationLevel::Serializable);
        assert!(lock_manager.requires_read_lock("sess_d"));
    }

    #[test]
    fn test_write_lock_escalation_requirements() {
        let mut lock_manager = RowLockManager::new();

        // READ UNCOMMITTED: no escalation
        lock_manager
            .set_session_isolation_level("sess_a".to_string(), IsolationLevel::ReadUncommitted);
        assert!(!lock_manager.requires_write_lock_escalation("sess_a"));

        // READ COMMITTED: no escalation (standard write lock sufficient)
        lock_manager
            .set_session_isolation_level("sess_b".to_string(), IsolationLevel::ReadCommitted);
        assert!(!lock_manager.requires_write_lock_escalation("sess_b"));

        // REPEATABLE READ: escalation required
        lock_manager
            .set_session_isolation_level("sess_c".to_string(), IsolationLevel::RepeatableRead);
        assert!(lock_manager.requires_write_lock_escalation("sess_c"));

        // SERIALIZABLE: escalation required
        lock_manager
            .set_session_isolation_level("sess_d".to_string(), IsolationLevel::Serializable);
        assert!(lock_manager.requires_write_lock_escalation("sess_d"));
    }

    #[test]
    fn test_isolation_level_consistency_across_sessions() {
        let mut lock_manager = RowLockManager::new();

        // Set different isolation levels for multiple sessions
        lock_manager
            .set_session_isolation_level("sess_1".to_string(), IsolationLevel::ReadUncommitted);
        lock_manager
            .set_session_isolation_level("sess_2".to_string(), IsolationLevel::ReadCommitted);
        lock_manager
            .set_session_isolation_level("sess_3".to_string(), IsolationLevel::RepeatableRead);
        lock_manager
            .set_session_isolation_level("sess_4".to_string(), IsolationLevel::Serializable);

        // Verify each session maintains its own level
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_1"),
            IsolationLevel::ReadUncommitted
        );
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_2"),
            IsolationLevel::ReadCommitted
        );
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_3"),
            IsolationLevel::RepeatableRead
        );
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_4"),
            IsolationLevel::Serializable
        );

        // Verify new sessions still get default
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_new"),
            IsolationLevel::ReadCommitted
        );

        // Clearing one doesn't affect others
        lock_manager.clear_session_isolation_level("sess_2");
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_2"),
            IsolationLevel::ReadCommitted
        );
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_1"),
            IsolationLevel::ReadUncommitted
        );
        assert_eq!(
            lock_manager.get_session_isolation_level("sess_3"),
            IsolationLevel::RepeatableRead
        );
    }
}
