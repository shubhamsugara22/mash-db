# MVCC Engine Foundation

## Status

Mash DB now has an in-memory MVCC store primitive in `src/persistence.rs`. It supports fixed-snapshot reads, transaction-local writes and deletes, monotonically increasing commit versions, and write-write conflict detection.

This is an engine foundation, not end-to-end MVCC for SQL. The REPL's `BEGIN`/`COMMIT`/`ROLLBACK` path does not use this store, data is not persisted through the WAL, and concurrent client sessions are not supported by the current executable.

## Semantics

- `begin()` captures the store's current commit version.
- `read(transaction, key)` first checks the transaction's private write set, then returns the newest committed key version visible at the transaction's snapshot.
- `write(key, value)` stages a private value; other transactions cannot see it before commit.
- `delete(key)` stages a tombstone. Transactions with older snapshots can still read the version that was visible to them.
- `commit(transaction)` rejects a key if it has a committed version newer than the transaction's snapshot. Writes to independent keys can commit independently.
- A successful transaction with writes receives one commit version shared by all of its writes. A read-only transaction does not advance the version.

This behavior is snapshot isolation with write-write conflict detection. It does not by itself provide serializable isolation: read-set validation, predicate/range tracking, and phantom prevention are not implemented.

## API Sketch

```rust
let mut store = MvccStore::<String, String>::default();
let mut transaction = store.begin();
transaction.write("item-1".to_string(), "value".to_string());
store.commit(transaction)?;

let reader = store.begin();
let value = store.read(&reader, &"item-1".to_string());
```

The `MvccCommitError` result distinguishes a write conflict (including key and versions) from commit-version overflow.

## Current Limitations

- In-memory only; committed values are lost when the process exits.
- Not connected to `Table`, SQL statements, or the existing transaction state.
- No WAL records or crash recovery for MVCC commits.
- No version garbage collection, so retained versions grow with updates/deletes.
- No read-set validation, predicate locks, or serializable/phantom protection.
- The executable remains a single-session REPL, not a concurrent database service.

Do not treat this primitive as a production persistence or concurrency guarantee.

## Tests

The MVCC unit tests cover snapshot visibility after a later commit, private uncommitted values, same-key conflict rejection, independent-key commits, and versioned deletion.

Run the focused tests:

```sh
cargo test persistence::tests::test_mvcc
```

Run the full suite before merging or releasing:

```sh
cargo test
```

## Next Integration Work

1. Define transaction and session ownership for actual concurrent SQL requests.
2. Route SQL reads and writes through MVCC snapshots and staged writes.
3. Make commits durable and recoverable through WAL/recovery logic.
4. Define version-retention/garbage-collection rules based on the oldest active snapshot.
5. Add read/write anomaly tests, including repeatable reads, write skew, and phantom behavior at each supported isolation level.
