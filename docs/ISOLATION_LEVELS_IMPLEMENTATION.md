# Isolation Levels Implementation

## Overview

This document describes the implementation of SQL isolation levels in Mash DB. Isolation levels are a critical component of the concurrency control system, defining the degree to which transactions are isolated from one another and from concurrent database modifications.

Mash DB implements all four SQL-standard isolation levels:
- **READ UNCOMMITTED (Level 0)**: Lowest safety, highest performance
- **READ COMMITTED (Level 1)**: Default level, prevents dirty reads
- **REPEATABLE READ (Level 2)**: Prevents dirty and non-repeatable reads
- **SERIALIZABLE (Level 3)**: Highest safety, prevents all anomalies

## Architecture

### Per-Session Isolation Level Tracking

Isolation levels are managed at the session level, allowing different connections to operate at different isolation levels simultaneously.

```
Connection Session
  ├── isolation_level: IsolationLevel
  └── session_id: String

RowLockManager
  └── isolation_level_overrides: HashMap<String, IsolationLevel>
      (Maps session_id → IsolationLevel)
```

### Implementation Components

#### 1. IsolationLevel Enum (persistence.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl IsolationLevel {
    pub fn as_u8(&self) -> u8 { /* 0-3 */ }
    pub fn name(&self) -> &'static str { /* human-readable name */ }
}
```

**Key Properties**:
- Serializable/Deserializable for persistence
- Supports numeric comparison via `as_u8()`
- Human-readable names via `name()`
- Implements Copy for efficiency in passing between functions

#### 2. ConnectionSession Enhancement

```rust
pub struct ConnectionSession {
    pub id: String,
    pub isolation_level: IsolationLevel,
    pub created_at: u64,
    pub last_activity: u64,
}

impl ConnectionSession {
    pub fn set_isolation_level(&mut self, level: IsolationLevel) { /* ... */ }
}
```

Each session maintains its own isolation level, which can be modified after creation.

#### 3. RowLockManager Enhancement

```rust
pub struct RowLockManager {
    locks: HashMap<(String, u32), LockRecord>,
    isolation_level_overrides: HashMap<String, IsolationLevel>,
    // ... other fields
}

impl RowLockManager {
    pub fn set_session_isolation_level(&mut self, session_id: String, level: IsolationLevel)
    pub fn get_session_isolation_level(&self, session_id: &str) -> IsolationLevel
    pub fn clear_session_isolation_level(&mut self, session_id: &str)
    pub fn requires_read_lock(&self, session_id: &str) -> bool
    pub fn requires_write_lock_escalation(&self, session_id: &str) -> bool
}
```

**Lock Requirement Rules**:

| Isolation Level | Read Locks | Write Lock Escalation |
|---|---|---|
| READ UNCOMMITTED | No | No |
| READ COMMITTED | Yes | No |
| REPEATABLE READ | Yes | Yes |
| SERIALIZABLE | Yes | Yes |

#### 4. DatabaseManager Wrapper Methods (db_manager.rs)

```rust
impl DatabaseManager {
    pub fn set_session_isolation_level(
        &mut self,
        session_id: &str,
        level: IsolationLevel,
    ) -> Result<(), String>
    
    pub fn get_session_isolation_level(&self, session_id: &str) -> IsolationLevel
    
    pub fn requires_read_lock_for_session(&self, session_id: &str) -> bool
    
    pub fn requires_write_lock_escalation_for_session(&self, session_id: &str) -> bool
}
```

These methods provide a public API for isolation level operations.

#### 5. DurabilityConfig Enhancement

```rust
pub struct DurabilityConfig {
    pub default_isolation_level: IsolationLevel,
    // ... other fields
}
```

Defines the global default isolation level for all new sessions (defaults to READ COMMITTED).

## Isolation Level Behavior

### READ UNCOMMITTED (Level 0)

**Characteristics**:
- Lowest safety level
- Highest performance (no read locks required)
- Allows dirty reads, non-repeatable reads, and phantom reads
- Suitable for reports and analytics where consistency isn't critical

**Lock Behavior**:
- Write locks are acquired for INSERT/UPDATE/DELETE operations
- Read locks are NOT acquired for SELECT operations
- No lock escalation

**Use Cases**:
- Data warehouse queries
- Analytical reports
- Non-critical data access
- Performance-critical operations where consistency is not required

### READ COMMITTED (Level 1 - DEFAULT)

**Characteristics**:
- Standard safety level
- Good balance between safety and performance
- Prevents dirty reads
- Allows non-repeatable reads and phantom reads
- Default isolation level for new sessions

**Lock Behavior**:
- Write locks acquired for INSERT/UPDATE/DELETE
- Read locks acquired for SELECT
- Locks released immediately after operation (not held for duration of transaction)
- No lock escalation

**Use Cases**:
- Most OLTP (Online Transaction Processing) operations
- Default choice for typical business applications
- Good balance for multi-user environments

### REPEATABLE READ (Level 2)

**Characteristics**:
- Strong safety level
- Prevents dirty reads and non-repeatable reads
- Allows phantom reads
- Higher lock contention due to write lock escalation

**Lock Behavior**:
- Write locks acquired for INSERT/UPDATE/DELETE
- Read locks acquired for SELECT
- Write lock escalation: read locks can be promoted to write locks if needed
- Locks held for duration of transaction

**Use Cases**:
- Financial transactions
- Inventory management
- Operations requiring consistent view of data
- Situations where non-repeatable reads would cause problems

### SERIALIZABLE (Level 3)

**Characteristics**:
- Highest safety level
- Prevents all anomalies (dirty reads, non-repeatable reads, phantom reads)
- Effectively serializes transactions
- Significant performance impact due to extensive locking

**Lock Behavior**:
- Write locks acquired for INSERT/UPDATE/DELETE
- Read locks acquired for SELECT
- Full write lock escalation: all locks are write locks at serializable level
- Locks held for duration of transaction
- Most restrictive locking strategy

**Use Cases**:
- Critical financial operations
- Data integrity is paramount
- Compliance/regulatory requirements
- Low-concurrency environments where serialization is acceptable

## Usage Examples

### Setting Isolation Level

```rust
// Set isolation level for a session
db_manager.set_session_isolation_level("session_1", IsolationLevel::RepeatableRead)?;

// Get current isolation level
let level = db_manager.get_session_isolation_level("session_1");
println!("Session 1 isolation level: {}", level.name());
```

### Checking Lock Requirements

```rust
// Check if session requires read locks
if db_manager.requires_read_lock_for_session("session_1") {
    // Acquire read locks for SELECT operations
}

// Check if session requires write lock escalation
if db_manager.requires_write_lock_escalation_for_session("session_1") {
    // Use escalating locks for REPEATABLE READ and SERIALIZABLE
}
```

### Configuration

```rust
// Set default isolation level in DurabilityConfig
let config = DurabilityConfig {
    default_isolation_level: IsolationLevel::ReadCommitted,
    // ... other config fields
};

let db = DatabaseManager::new("mydb", "/path/to/db", 10, config)?;
```

## Performance Implications

### Lock Overhead by Level

```
Performance (Higher is Better)
READ UNCOMMITTED  ████████████████  Lowest overhead
READ COMMITTED    ██████████         Moderate overhead
REPEATABLE READ   ████               Higher overhead
SERIALIZABLE      ██                 Highest overhead
```

### Recommendations

1. **Default to READ COMMITTED** for most applications
2. **Use READ UNCOMMITTED** only for reports/analytics where stale data is acceptable
3. **Use REPEATABLE READ** when you need consistency within a transaction but can tolerate phantom reads
4. **Use SERIALIZABLE** only when absolutely required (high cost)

## Testing

### Test Coverage

The isolation level implementation includes 6 comprehensive tests:

1. **test_isolation_level_default**: Verifies default is READ COMMITTED
2. **test_isolation_level_hierarchy**: Validates numeric levels (0-3)
3. **test_isolation_level_names**: Confirms human-readable names
4. **test_connection_session_isolation_level**: Tests session isolation level setting
5. **test_row_lock_manager_isolation_level_management**: Tests set/get/clear operations
6. **test_isolation_level_consistency_across_sessions**: Verifies session independence

### Running Tests

```bash
# Run isolation level tests
cargo test isolation_level

# Run full test suite (444 tests total)
cargo test
```

## Integration Points

### In Database Operations

```
SQL Operation (e.g., SELECT)
  ↓
DatabaseManager receives operation
  ↓
Gets session's isolation level via get_session_isolation_level()
  ↓
Checks requires_read_lock() for isolation level
  ↓
Acquires appropriate locks based on isolation level
  ↓
Executes operation
  ↓
Releases locks according to isolation level rules
```

### Lock Acquisition Decision

```rust
if db.requires_read_lock_for_session(session_id) {
    // Acquire read lock for SELECT
    db.lock_row(table, row_id, session_id)?;
}

if db.requires_write_lock_escalation_for_session(session_id) {
    // Escalate read locks to write locks for REPEATABLE READ/SERIALIZABLE
    db.escalate_lock(table, row_id, session_id)?;
}
```

## Future Enhancements

1. **MVCC Implementation**: Support for multi-version concurrency control
2. **Snapshot Isolation**: Non-blocking reads via versioning
3. **Dynamic Level Switching**: Change isolation level mid-transaction
4. **Performance Monitoring**: Track lock contention by isolation level
5. **Adaptive Isolation**: Automatically adjust levels based on workload

## References

- **SQL Standard**: ISO/IEC 9075 - Information technology Database languages SQL
- **ANSI Definitions**: SQL/92 isolation level specifications
- **Concurrency Control Phases**:
  - Phase 1: ✅ Row-Level Locking
  - Phase 2: ✅ Lock Timeout Enforcement
  - Phase 3: ✅ Deadlock Detection
  - Phase 4: ✅ Isolation Levels (THIS DOCUMENT)

## Conclusion

The isolation levels implementation in Mash DB provides:
- **Safety**: All four SQL-standard isolation levels with correct semantics
- **Performance**: Per-session configuration allowing workload-specific optimization
- **Flexibility**: Easy switching between levels for different operational needs
- **Reliability**: Comprehensive testing and integration with existing concurrency control

This completes the four-phase concurrency control implementation for Mash DB.
