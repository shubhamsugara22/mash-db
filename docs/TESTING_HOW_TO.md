# Testing How-To

This guide explains how to check, test, and smoke-test Mash DB on Windows (PowerShell 5.1).

## 1. Compile check

```powershell
cargo check
```

Expected: `0 errors`. Warnings are allowed. The project currently reports about 23 warnings in the binary build, including one unused-import suggestion that has not been addressed.

## 2. Build

```powershell
cargo build
```

The debug binary is written to `target\debug\Mash_db.exe`.

## 3. Automated tests

```powershell
cargo test
```

Last verified run: 484 unit tests and 1 integration test, all passing, 0 failed.

To run a subset, pass a filter, for example:

```powershell
cargo test backup
cargo test join
```

### Reading logs on Windows

Cargo output redirected with `>` from PowerShell is UTF-16 encoded. Read it with:

```powershell
Get-Content check_final.log -Encoding Unicode | Select-String -Pattern '^error'
Get-Content test_final.log -Encoding Unicode | Select-String -Pattern 'test result|FAILED|panicked'
```

Opening these files as plain text shows raw bytes.

## 4. Smoke test the REPL

Run the smoke test in a temporary copy so that the workspace `auth.json`, `data\`, and `schemas.json` are not changed.

```powershell
$t = Join-Path $env:TEMP 'mash_smoke'
Remove-Item -Recurse -Force $t -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $t | Out-Null
Copy-Item target\debug\Mash_db.exe $t
Set-Location $t
```

Create an input file (`input.txt`) with one statement per line:

```text
CREATE USER admin PASSWORD secret123 ROLE admin
LOGIN admin secret123
CREATE TABLE products (id, name, price, stock, category)
INSERT INTO products VALUES (1, 'Widget', '19.99', '100', 'Tools')
INSERT INTO products VALUES (2, 'Gadget', '5.50', '0', 'Toys')
SELECT * FROM products
SELECT category, SUM(stock) FROM products GROUP BY category
BEGIN
INSERT INTO products VALUES (3, 'Temp', '1', '1', 'X')
ROLLBACK
SELECT COUNT(*) FROM products
BACKUP
DELETE FROM products WHERE id = 1
SELECT * FROM products
.exit
```

Run it:

```powershell
Get-Content input.txt | .\Mash_db.exe
```

Expected results:

- The first `CREATE USER` creates the admin account. The first account bootstraps the auth catalog.
- `ROLLBACK` removes the row inserted inside the transaction, so `COUNT(*)` returns 2.
- `BACKUP` creates a snapshot. `DELETE` removes row 1.
- `RESTORE TO <unix_seconds>` restores the newest full snapshot at or before the target. Use a timestamp at or after the backup time. For example, compute it with `[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()`.

### Known cosmetic issues

- The restore success message prints `at .` with no timestamp.
- Login prints a `DEBUG handle_session: Login` line to stderr.

Neither affects behavior.

## 5. Where test artifacts live

- `test_backups*/`, `test_db_path*/`, and `Test-commands/` are sample and test fixtures in the workspace. Do not delete them without checking first.
- Running the REPL in the workspace writes to `data\`, `auth.json`, and schema files. Use the temp-copy method above for experiments.
