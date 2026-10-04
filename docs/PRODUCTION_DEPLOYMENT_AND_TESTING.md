# Production Deployment and Testing Guide

## Scope and Deployment Boundary

Mash DB is currently a local, interactive Rust REPL backed by files. It is not a network database server and does not expose a TCP/HTTP API, a service protocol, or a supported multi-process access model. Run one process against one database directory at a time. Do not put the files on a shared directory and access them from multiple hosts or processes.

The source documentation also describes the engine as single-threaded and single-user. Treat the connection pool, row-locking, and isolation-level APIs as internal components, not as evidence that concurrent production clients are supported. In particular, the REPL does not currently call the `requires_read_lock_for_session` or `requires_write_lock_escalation_for_session` APIs to govern its SQL reads and writes. Validate this boundary against the exact release you deploy.

Use this guide for a controlled, single-instance pilot or internal workload. Do not use this build for a critical or multi-user workload until the service interface, concurrency model, recovery behavior, and operational requirements have been independently validated.

## Runtime Data Layout

The executable uses the current working directory for table and catalog files. The persistence manager uses a `data/` subdirectory. Keep both locations together and back up both:

- Top-level table JSON files, including `data.json` and files for other tables
- `schemas.json`
- `auth.json` (contains account and credential data; restrict its access)
- The full `data/` directory, including WAL, metadata, audit log, and backups

Always start Mash DB with the intended database directory as its working directory. Use one isolated directory per environment (development, staging, production). Do not copy production credentials into a test environment; scrub or replace them first.

## Prerequisites

- Rust toolchain compatible with the Rust 2021 edition
- Cargo and access to download the locked dependencies during the build
- A dedicated OS account with read/write access only to the deployment and data directories
- Adequate disk space, monitoring, and a tested backup destination

Build and test from a clean source checkout or CI workspace. Do not run the test suite in the live database working directory.

## Build and Verify a Release

Run the full test suite before packaging:

```powershell
cargo test
```

The repository's recent baseline was 444 unit tests plus 1 integration test. Treat the current command's result as authoritative if the suite changes. Any failing test blocks release until investigated.

Build an optimized binary:

```powershell
cargo build --release
```

On Windows, the executable is normally `target\release\Mash_db.exe`. On Linux/macOS it is normally `target/release/Mash_db`. Build for the same OS and architecture as the target host; do not copy a Windows binary to Linux or vice versa. Record the source revision, Rust toolchain version, build result, and artifact checksum with the release.

Optional Windows checksum:

```powershell
Get-FileHash .\target\release\Mash_db.exe -Algorithm SHA256
```

Optional Linux/macOS checksum:

```sh
sha256sum target/release/Mash_db
```

## Staging Validation

Create a staging directory with a copy of the release artifact and a separate, disposable data set. Start the executable with that directory as its working directory. Verify all of the following before production rollout:

1. The process starts and initializes its persistence files without errors.
2. Create a test table, insert a uniquely identifiable test row, query it, update it, and delete it. Use only disposable staging data.
3. Exit with `.exit`, restart the process from the same working directory, and confirm expected data and schemas remain available.
4. Verify authentication and permissions using non-production test accounts if those features are in scope.
5. Exercise backup and restore using a staging backup, then verify restored rows and schema.
6. Review logs and generated files for errors, unexpected data paths, and accidental exposure of credentials.
7. Run the full automated suite against the release source/artifact in an isolated CI workspace.

Do not use a production instance for destructive smoke tests. The SQL REPL is interactive; it is not a batch server endpoint.

## Production Rollout

1. Schedule a maintenance window. Stop all Mash DB processes using the target data set and confirm no second instance is running.
2. Record the current application artifact/version and the current working directory.
3. Make a consistent, offline backup of all top-level database JSON files, `schemas.json`, `auth.json`, and the complete `data/` directory. Store the backup outside the deployment directory, restrict its permissions, and verify that it can be read.
4. Deploy the tested release binary without overwriting the database files. Preserve the same working-directory layout and ownership/permissions.
5. Start the executable from the designated production working directory. Confirm startup completes and that it is using the intended files.
6. Run only non-destructive smoke checks, such as selecting known records and checking table schemas. Do not insert synthetic records or run DDL against live data as a smoke test.
7. Monitor process exit, disk space, audit/WAL growth, and application output during the agreed observation period. Keep an operator available to stop the process if behavior differs from staging.

Keep production credentials out of source control, deployment logs, and test fixtures. Limit access to `auth.json`, backups, audit logs, and the data directory through OS permissions and your organization's secret-handling procedures.

## Backup and Restore Policy

- Back up the top-level table/catalog files and the entire `data/` directory as one consistent set while the process is stopped.
- Keep backups encrypted or stored on an access-controlled destination according to organizational policy.
- Retain at least one verified backup outside the host that runs Mash DB.
- Perform restore drills in an isolated directory. Never test a restore over the only production copy.
- Record the restore point and the expected recovery point/time before any planned change.

The executable's live data spans multiple locations. A backup of only `data/` or only the top-level JSON files is incomplete.

## Rollback

1. Stop Mash DB and ensure no process is writing to the database.
2. If the new binary is the only change and data formats remain compatible, restore the prior binary while preserving current data, then validate using read-only queries.
3. If data files were changed, corrupted, or are incompatible, restore the complete pre-deployment snapshot (all top-level database files plus `data/`) into the intended working directory. This discards writes made after that snapshot; obtain the data owner's approval before doing so.
4. Start the prior release and perform the same non-destructive checks used after rollout.
5. Preserve the failed artifact, logs, and a copy of the affected data for investigation; do not overwrite the only evidence.

## Release Acceptance Checklist

- [ ] All automated tests pass in an isolated checkout.
- [ ] Release binary builds for the target OS/architecture and checksum is recorded.
- [ ] Staging CRUD, restart persistence, authentication (if used), and backup/restore checks pass.
- [ ] Production is configured as a single instance with a private local data directory.
- [ ] A complete, offline, verified pre-deployment backup exists outside the host.
- [ ] Rollback owner, maintenance window, and data-loss decision process are agreed.
- [ ] The workload does not require unsupported network clients or concurrent multi-user access.

If the final condition is false, do not deploy this build as the production database for that workload. Use a database server designed for the required client concurrency and availability, or first implement and validate the missing service and concurrency guarantees.
