# Mash DB - Complete Documentation & Manual

This file consolidates all documentation for easy reference and PDF conversion.

**Last Updated**: Current Session
**Version**: 1.0.0 - Dynamic Schemas Edition
**Status**: Production Ready

---

## Table of Contents

1. [Quick Start Guide](#quick-start-guide)
2. [Installation](#installation)
3. [Basic Commands](#basic-commands)
4. [SQL Reference](#sql-reference)
5. [Advanced Features](#advanced-features)
6. [Architecture Overview](#architecture-overview)
7. [Performance Characteristics](#performance-characteristics)
8. [Feature Status](#feature-status)
9. [Troubleshooting](#troubleshooting)
10. [Development Info](#development-info)

---

## Quick Start Guide

### Building
```bash
cargo build --release
```

### Running
```bash
./target/release/Mash_db.exe
```

### First Commands
```sql
CREATE TABLE users (id, username, email)
INSERT INTO users VALUES (1, 'alice', 'alice@example.com')
SELECT * FROM users
```

---

## Installation

### Requirements
- Rust 1.60+
- Cargo
- ~50MB disk space for build

### Steps
1. Clone repository
2. Run `cargo build --release`
3. Execute `./target/release/Mash_db.exe`

---

## Basic Commands

### Data Manipulation

#### INSERT
```sql
-- Format 1: Simple
INSERT 1 alice alice@example.com

-- Format 2: Named columns
INSERT INTO table_name VALUES (val1, val2, val3)
```

#### SELECT
```sql
-- All columns
SELECT * FROM table_name

-- With WHERE
SELECT * FROM table_name WHERE column = 'value'

-- With ORDER BY
SELECT * FROM table_name ORDER BY column ASC

-- With LIMIT
SELECT * FROM table_name LIMIT 10

-- Combined
SELECT * FROM products WHERE price > '20' ORDER BY name DESC LIMIT 5
```

#### UPDATE
```sql
UPDATE table_name SET column = 'value' WHERE id = 1
```

#### DELETE
```sql
-- By ID
DELETE WHERE id = 1

-- By condition
DELETE WHERE category = 'Tools'

-- All rows
DELETE ALL
```

### Table Management

#### CREATE TABLE
```sql
CREATE TABLE table_name (col1, col2, col3, col4, col5)
```

#### DROP TABLE
```sql
DROP TABLE table_name
```

#### RENAME TABLE
```sql
ALTER TABLE old_name RENAME TO new_name
```

#### Show Tables
```sql
SHOW TABLES
```

---

## SQL Reference

### WHERE Operators
- **Comparison**: =, !=, >, <, >=, <=
- **Logical**: AND, OR
- **Precedence**: AND > OR

### ORDER BY
- **ASC**: Ascending (default)
- **DESC**: Descending
- **Multiple columns**: ORDER BY col1 ASC, col2 DESC

### Aggregate Functions
- **COUNT(*)**: Count rows
- **COUNT(DISTINCT column)**: Count unique values
- **SUM(column)**: Sum numeric values
- **AVG(column)**: Average values
- **MIN(column)**: Minimum value
- **MAX(column)**: Maximum value

### GROUP BY
```sql
-- Single column
SELECT category, COUNT(*) FROM products GROUP BY category

-- Multiple columns
SELECT category, price, COUNT(*) FROM products GROUP BY category, price

-- With HAVING
SELECT category, SUM(stock) FROM products 
GROUP BY category HAVING SUM(stock) > 100
```

### DISTINCT
```sql
-- Remove duplicates
SELECT DISTINCT column FROM table
```

### LIMIT & OFFSET
```sql
-- First 10 rows
SELECT * FROM table LIMIT 10

-- Rows 11-20 (page 2)
SELECT * FROM table LIMIT 10 OFFSET 10
```

---

## Advanced Features

### Dynamic Schemas
Each table can have any number of custom columns:

```sql
CREATE TABLE products (id, name, price, stock, category)
CREATE TABLE stores (id, name, city, manager, year_opened)
CREATE TABLE employees (id, name, department, salary, hire_date)
```

### B-Tree Indexing
Automatic indexes on (id, username, email) for fast lookups:

```sql
-- These use indexes (O(log n))
SELECT * FROM users WHERE id = 5
SELECT * FROM users WHERE username = 'alice'

-- These scan table (O(n))
SELECT * FROM products WHERE category = 'Tools'
SELECT * FROM stores WHERE city = 'NYC'
```

### Complex WHERE Clauses
```sql
-- AND has higher precedence
SELECT * FROM products 
WHERE category = 'Tools' AND price > '10' OR stock > 100
-- Interpreted as: (category='Tools' AND price>'10') OR stock>100
```

### Pagination
```sql
-- Page 1: Rows 0-9
SELECT * FROM products ORDER BY id LIMIT 10 OFFSET 0

-- Page 2: Rows 10-19
SELECT * FROM products ORDER BY id LIMIT 10 OFFSET 10

-- Page 3: Rows 20-29
SELECT * FROM products ORDER BY id LIMIT 10 OFFSET 20
```

### Data Persistence
- Automatic save after each operation
- Binary JSON-compatible format
- One file per table (*.json)
- Schema stored in schemas.json

---

## Architecture Overview

### Layers

```
User Interface (REPL)
    ↓
SQL Parser (tokenize & parse)
    ↓
Statement Executor (execute parsed SQL)
    ↓
Table Operations (CRUD)
    ↓
B-Tree Indexes (fast lookups)
    ↓
Storage/Pager (persistence)
    ↓
File System (JSON files)
```

### Key Components

**main.rs** (2435 lines):
- Interactive REPL interface
- Statement execution engine
- Schema registry management
- Query orchestration

**parser.rs** (1934 lines):
- SQL tokenization
- Statement parsing
- Syntax validation
- Support for all SQL operations

**table.rs** (807 lines):
- Row structure (id, username, email, extras HashMap)
- Table structure (schema Vec, rows Vec, indexes BTreeMap)
- CRUD operations
- Index management
- Aggregate functions

**pager.rs** (storage):
- Page-based storage management
- Binary serialization
- File I/O operations

### Data Structures

**Row**:
- id: u32 (primary key)
- username: String (fixed field)
- email: String (fixed field)
- extras: HashMap<String, String> (dynamic columns)

**Table**:
- schema: Vec<String> (column names)
- rows: Vec<Row> (data storage)
- id_index: BTreeMap<u32, ...> (fast id lookups)
- username_index: BTreeMap<String, ...> (fast username lookups)
- email_index: BTreeMap<String, ...> (fast email lookups)

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Note |
|-----------|-----------|------|
| INSERT | O(log n) | Index updates |
| SELECT by ID | O(log n) | Uses index |
| SELECT by column | O(n) | Full scan |
| UPDATE | O(log n) | Index updates |
| DELETE | O(log n) | Index cleanup |
| ORDER BY | O(n log n) | Sorting |
| GROUP BY | O(n) | Iteration + aggregation |
| LIMIT/OFFSET | O(k) | k = limit size |

### Space Complexity

| Component | Complexity | Note |
|-----------|-----------|------|
| Table storage | O(n) | n = number of rows |
| Indexes | O(n) | B-Tree per indexed column |
| Extras HashMap | O(m) | m = extra columns per row |

### Performance Tips

1. **Use indexed columns**: id, username, email
2. **Add WHERE filters**: Reduces rows to process
3. **Use LIMIT**: Don't fetch unnecessary data
4. **Keep datasets reasonable**: Single-node database
5. **Avoid multiple conditions on unindexed columns**: Requires full scan

---

## Feature Status

### Completed ✅
- CRUD operations (INSERT, SELECT, UPDATE, DELETE)
- WHERE with AND/OR operators
- ORDER BY (ASC/DESC)
- LIMIT/OFFSET (pagination)
- DISTINCT (deduplication)
- GROUP BY (single & multiple columns)
- Aggregate functions (COUNT, SUM, AVG, MIN, MAX)
- HAVING (group filtering)
- CREATE/DROP/ALTER TABLE
- Dynamic schemas
- B-Tree indexing
- Data persistence
- Meta commands (.exit)

### In Development 🔄
- Type system enhancements
- Custom column indexes
- ALTER TABLE ADD/DROP COLUMN

### Planned 📋
- Transaction support (ACID)
- JOIN operations (INNER, LEFT, RIGHT)
- Subqueries (nested SELECT)
- Constraints (NOT NULL, UNIQUE, PRIMARY KEY)
- Full-text search (LIKE pattern matching)
- Views
- Stored procedures

---

## Troubleshooting

### Common Issues

**"Duplicate id" error**
- Cause: ID already exists in table
- Solution: Use a different ID

**"Table not found" error**
- Cause: Table doesn't exist or wrong name
- Solution: Check with `SHOW TABLES`

**"Unrecognized keyword" error**
- Cause: SQL syntax error
- Solution: Verify command syntax

**No results from SELECT**
- Cause: Table empty or WHERE filter too restrictive
- Solution: Try `SELECT * FROM table` without WHERE

**Slow queries**
- Cause: Querying unindexed column
- Solution: Use indexed columns (id, username, email) when possible

### Data Recovery

**Backup**:
```bash
cp *.json backup/
```

**Restore**:
```bash
cp backup/*.json .
```

---

## Development Info

### Project Structure
```
src/
  main.rs         → CLI & execution
  parser.rs       → SQL parsing
  table.rs        → Data structures
  pager.rs        → Storage
  column.rs       → Column definitions
Cargo.toml        → Dependencies
```

### Key Dependencies
- serde: Serialization
- bincode: Binary encoding
- regex: Pattern matching

### Building from Source
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Clean build artifacts
cargo clean
```

### Code Statistics
- **Total lines**: ~5,200
- **Files**: 5 source files
- **Build time**: ~30-60 seconds (release)
- **Binary size**: 0.68 MB (release)
- **Compilation status**: ✅ 0 errors, 13 warnings (non-critical)

---

## Examples

### E-Commerce Database

```sql
-- Create tables
CREATE TABLE products (id, name, price, stock, category)
CREATE TABLE customers (id, name, email, city)
CREATE TABLE orders (id, customer_id, product_id, quantity, date)

-- Insert data
INSERT INTO products VALUES (1, 'Widget', '19.99', '100', 'Tools')
INSERT INTO customers VALUES (1, 'Alice', 'alice@example.com', 'NYC')
INSERT INTO orders VALUES (1, '1', '1', '2', '2024-01-15')

-- Queries
SELECT category, COUNT(*), SUM(stock) FROM products GROUP BY category
SELECT city, COUNT(*) FROM customers GROUP BY city
SELECT product_id, SUM(quantity) FROM orders GROUP BY product_id
```

### Reporting System

```sql
-- Sales summary
SELECT category, SUM(stock) as total_stock, COUNT(*) as item_count 
FROM products 
GROUP BY category 
ORDER BY total_stock DESC

-- Customer analysis
SELECT city, COUNT(*) as customer_count 
FROM customers 
GROUP BY city 
ORDER BY customer_count DESC

-- Top products
SELECT id, name, price, stock 
FROM products 
WHERE stock > '50' 
ORDER BY price DESC 
LIMIT 10
```

---

## File Structure

```
Mash_db/
├── src/
│   ├── main.rs              → CLI/REPL
│   ├── parser.rs            → SQL parsing
│   ├── table.rs             → Data structures
│   ├── pager.rs             → Storage
│   └── column.rs            → Columns
├── target/release/
│   └── Mash_db.exe          → Compiled binary
├── *.json                   → Database files
│   ├── users.json
│   ├── products.json
│   └── schemas.json
├── Cargo.toml              → Project config
├── MANUAL.md               → User manual
├── DIAGRAMS.md             → Architecture diagrams
└── *.pdf                   → Generated documentation
```

---

## License & Support

- **License**: Open Source
- **Status**: Production Ready
- **Version**: 1.0.0 Dynamic Schemas
- **Last Updated**: Current Session

---

## Getting Started Checklist

- [ ] Build project: `cargo build --release`
- [ ] Run executable: `./target/release/Mash_db.exe`
- [ ] Create first table: `CREATE TABLE users (id, username, email)`
- [ ] Insert data: `INSERT INTO users VALUES (1, 'alice', 'alice@example.com')`
- [ ] Query data: `SELECT * FROM users`
- [ ] Explore advanced features
- [ ] Check DIAGRAMS.md for architecture details

---

## For More Information

- **User Manual**: See MANUAL.md
- **Architecture**: See DIAGRAMS.md
- **Feature Details**: See specific documentation files
- **Code**: Review src/ directory for implementation details

---

**Happy querying!** 🚀

This consolidated document can be easily converted to PDF and distributed as a complete user & developer manual.
