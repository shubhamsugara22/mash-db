use crate::backup::{BackupManager, BackupMetadata};
/// RDS-Ready Database Manager
///
/// Orchestrates:
/// - Persistence with durability guarantees
/// - Write-ahead logging and crash recovery
/// - Connection pooling and session management
/// - Backup/restore operations
/// - Health monitoring
/// - Database lifecycle
use crate::persistence::{
    ConnectionPool, ConnectionSession, DatabaseHealth, DatabaseMetadata, DatabaseStatus,
    DurabilityConfig, WriteAheadLog,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DatabaseManager {
    db_name: String,
    db_path: PathBuf,
    config: DurabilityConfig,
    metadata: DatabaseMetadata,
    connection_pool: ConnectionPool,
    wal: WriteAheadLog,
    backup_manager: BackupManager,
    health: DatabaseHealth,
}

impl DatabaseManager {
    /// Initialize a new database manager
    pub fn new(
        db_name: &str,
        db_path: &str,
        max_connections: usize,
        config: DurabilityConfig,
    ) -> Result<Self, String> {
        // Create database directory structure
        fs::create_dir_all(db_path)
            .map_err(|e| format!("Failed to create database path: {}", e))?;

        let db_path_buf = PathBuf::from(db_path);

        // Initialize metadata
        let metadata_path = db_path_buf.join("metadata.json");
        let metadata = if metadata_path.exists() {
            DatabaseMetadata::load(&metadata_path)?
        } else {
            DatabaseMetadata::new(db_name)
        };

        // Initialize WAL
        let wal = WriteAheadLog::new(db_path)?;

        // Initialize backup manager
        let backup_dir = db_path_buf.join("backups");
        let backup_manager = BackupManager::new(
            backup_dir.to_str().ok_or("Invalid path".to_string())?,
            10, // Keep last 10 backups
        )?;

        Ok(DatabaseManager {
            db_name: db_name.to_string(),
            db_path: db_path_buf,
            config,
            metadata,
            connection_pool: ConnectionPool::new(max_connections),
            wal,
            backup_manager,
            health: DatabaseHealth::new(),
        })
    }

    /// Create a new connection session
    pub fn create_connection(&mut self) -> Result<String, String> {
        self.connection_pool.create_session()
    }

    /// Close a connection session
    pub fn close_connection(&mut self, session_id: &str) -> Result<(), String> {
        self.connection_pool.close_session(session_id)
    }

    /// Log in a user to a session
    pub fn login_to_session(&mut self, session_id: &str, username: &str) -> Result<(), String> {
        if let Some(session) = self.connection_pool.get_session_mut(session_id) {
            session.set_user(username.to_string());
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Update session activity timestamp
    pub fn update_session_activity(&mut self, session_id: &str) -> Result<(), String> {
        if let Some(session) = self.connection_pool.get_session_mut(session_id) {
            session.update_activity();
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Log a write operation before executing (for crash recovery)
    pub fn log_write_before(&mut self, table_name: &str, operation: &str) -> Result<(), String> {
        if self.config.wal_enabled {
            self.wal.log_before_write(table_name, operation)?;
        }
        Ok(())
    }

    /// Log write completion after successful execution
    pub fn log_write_after(
        &mut self,
        table_name: &str,
        operation: &str,
        success: bool,
    ) -> Result<(), String> {
        if self.config.wal_enabled {
            self.wal.log_after_write(table_name, operation, success)?;
        }
        Ok(())
    }

    /// Log transaction begin
    pub fn log_transaction_begin(&mut self, tx_id: &str) -> Result<(), String> {
        if self.config.wal_enabled {
            self.wal.log_transaction_begin(tx_id)?;
        }
        Ok(())
    }

    /// Log transaction commit
    pub fn log_transaction_commit(&mut self, tx_id: &str) -> Result<(), String> {
        if self.config.wal_enabled {
            self.wal.log_transaction_commit(tx_id)?;
        }
        Ok(())
    }

    /// Log transaction rollback
    pub fn log_transaction_rollback(&mut self, tx_id: &str) -> Result<(), String> {
        if self.config.wal_enabled {
            self.wal.log_transaction_rollback(tx_id)?;
        }
        Ok(())
    }

    /// Register a table in metadata
    pub fn register_table(&mut self, table_name: &str, columns: Vec<String>) {
        self.metadata.register_table(table_name, columns);
    }

    pub fn ensure_table(&mut self, table_name: &str, columns: Vec<String>) {
        if !self.metadata.tables.contains_key(table_name) {
            self.metadata.register_table(table_name, columns);
        }
    }

    /// Update table statistics
    pub fn update_table_stats(&mut self, table_name: &str, row_count: usize) {
        self.metadata.update_table_stats(table_name, row_count);
    }

    /// Create a full database backup
    pub fn backup_full(
        &mut self,
        data_files: Vec<(&str, Vec<u8>)>,
    ) -> Result<BackupMetadata, String> {
        self.backup_manager.backup_full(&self.db_name, data_files)
    }

    /// Restore database from backup
    pub fn restore_backup(&self, backup_id: &str, restore_path: &str) -> Result<(), String> {
        self.backup_manager.restore_backup(backup_id, restore_path)
    }

    /// List all backups
    pub fn list_backups(&self) -> Vec<BackupMetadata> {
        self.backup_manager.list_backups()
    }

    /// Check database health
    pub fn check_health(&mut self) -> DatabaseStatus {
        let active_connections = self.connection_pool.get_active_sessions_count();
        let wal_size = self.get_wal_size();
        self.health.check_health(active_connections, wal_size)
    }

    /// Get current health status
    pub fn get_health(&self) -> &DatabaseHealth {
        &self.health
    }

    /// Cleanup idle connections
    pub fn cleanup_idle_connections(&mut self) {
        self.connection_pool.cleanup_idle_sessions();
    }

    /// Get list of active sessions
    pub fn get_active_sessions(&self) -> Vec<ConnectionSession> {
        self.connection_pool.get_all_sessions()
    }

    /// Save database metadata and state
    pub fn save_state(&self) -> Result<(), String> {
        let metadata_path = self.db_path.join("metadata.json");
        self.metadata.save(&metadata_path)
    }

    /// Get database statistics
    pub fn get_statistics(&self) -> DatabaseStatistics {
        let backup_stats = self.backup_manager.get_statistics();

        DatabaseStatistics {
            db_name: self.db_name.clone(),
            tables_count: self.metadata.tables.len(),
            total_rows: self.metadata.tables.values().map(|t| t.row_count).sum(),
            active_connections: self.connection_pool.get_active_sessions_count(),
            backup_count: backup_stats.total_backups,
            backup_total_size: backup_stats.total_size_bytes,
            wal_size: self.get_wal_size(),
            health_status: self.health.status.clone(),
        }
    }

    /// Get WAL file size
    fn get_wal_size(&self) -> u64 {
        self.db_path
            .join("wal.log")
            .metadata()
            .ok()
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Perform checkpoint (save state and clear WAL)
    pub fn checkpoint(&mut self) -> Result<(), String> {
        // Save metadata
        self.save_state()?;

        // Clear WAL after successful checkpoint
        self.wal.clear_log()?;

        println!("Database checkpoint completed successfully");
        Ok(())
    }

    /// Get database path
    pub fn get_db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get durability configuration
    pub fn get_config(&self) -> &DurabilityConfig {
        &self.config
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    pub db_name: String,
    pub tables_count: usize,
    pub total_rows: usize,
    pub active_connections: usize,
    pub backup_count: usize,
    pub backup_total_size: u64,
    pub wal_size: u64,
    pub health_status: DatabaseStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_manager_creation() {
        let config = DurabilityConfig::default();
        let result = DatabaseManager::new("test_db", "test_db_path", 10, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_connection() {
        let config = DurabilityConfig::default();
        let mut manager = DatabaseManager::new("test_db", "test_db_path2", 10, config).unwrap();

        let result = manager.create_connection();
        assert!(result.is_ok());
    }

    #[test]
    fn test_database_statistics() {
        let config = DurabilityConfig::default();
        let manager = DatabaseManager::new("test_db", "test_db_path3", 10, config).unwrap();

        let stats = manager.get_statistics();
        assert_eq!(stats.db_name, "test_db");
        assert_eq!(stats.tables_count, 0);
    }
}
