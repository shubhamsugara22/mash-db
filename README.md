# Mash DB

A simple database implementation in Rust, built from scratch following SQLite architecture.

## Current Features

- **REPL Interface** - Interactive command-line interface with `db >` prompt
- **Meta Commands** - `.exit` to quit the database, `.save <filename>` to save data, `.load <filename>` to load data
- **Basic SQL Support**:
  - `INSERT INTO table VALUES (id, 'username', 'email')` - Insert a row
  - `SELECT * FROM table` - Retrieve all rows
  - `SELECT * FROM table WHERE column = 'value'` - Select rows with WHERE condition
  - `SELECT DISTINCT column FROM table` - Select unique values
  - `UPDATE table SET column = 'value' WHERE id = number` - Update a row
  - `DELETE FROM table WHERE id = number` - Delete a row by ID
  - `DELETE FROM table WHERE column = 'value'` - Delete rows with WHERE condition
- **Advanced SQL Features**:
  - **Aggregate Functions**: `COUNT(*)`, `COUNT(col)`, `COUNT(DISTINCT col)`, `SUM(col)`, `AVG(col)`, `MIN(col)`, `MAX(col)`
  - **GROUP BY**: Group results by one or multiple columns (e.g., `GROUP BY username, email`)
  - **HAVING**: Filter grouped results with conditions (e.g., `HAVING COUNT(*) > 1`)
  - **ORDER BY**: Sort results ascending or descending (e.g., `ORDER BY username DESC`)
  - **LIMIT/OFFSET**: Paginate results (e.g., `LIMIT 10 OFFSET 5`)
  - **JOIN Operations**: INNER, LEFT, and RIGHT JOIN support
    - `SELECT * FROM users INNER JOIN orders ON id = id`
    - `SELECT * FROM users LEFT JOIN orders ON username = username`
    - `SELECT * FROM users RIGHT JOIN orders ON id = id`
- **B-Tree Indexing** - Efficient O(log n) lookups for ID-based operations, and fast lookups for username and email
- **In-Memory Storage** - Table stores rows in a Vec with B-Tree indexes
- **Data Validation** - Username (max 32 chars) and email (max 255 chars) length checks
- **Persistence** - Save/load table to/from JSON files
- **Comprehensive Testing** - 86 passing tests covering all SQL features

## Usage

```bash
cargo run
```

Example session:
```
db > INSERT INTO users VALUES (1, 'alice', 'alice@example.com')
Executed.
db > INSERT INTO users VALUES (2, 'bob', 'bob@example.com')
Executed.
db > INSERT INTO users VALUES (3, 'alice', 'alice2@example.com')
Executed.
db > SELECT * FROM users
(1, alice, alice@example.com)
(2, bob, bob@example.com)
(3, alice, alice2@example.com)
Executed.
db > SELECT username, COUNT(*), COUNT(DISTINCT email) FROM users GROUP BY username
alice, 2, 2
bob, 1, 1
Executed.
db > SELECT username FROM users GROUP BY username HAVING COUNT(*) > 1
alice
Executed.
db > SELECT * FROM users ORDER BY username DESC LIMIT 2
(2, bob, bob@example.com)
(3, alice, alice2@example.com)
Executed.
db > SELECT * FROM users INNER JOIN orders ON id = id
(1, alice, alice@example.com)
(2, bob, bob@example.com)
Executed.
db > SELECT * FROM users LEFT JOIN orders ON username = username
(1, alice, alice@example.com)
(2, bob, bob@example.com)
(3, alice, alice2@example.com)
Executed.
db > .exit
Bye!
```

## Architecture

- `main.rs` - REPL loop, command parsing, and statement execution
- `table.rs` - Row and Table structures for data storage with B-Tree indexes for id, username, and email
- `column.rs` - (Reserved for future column definitions)

## TODO

- [x] Persistence (save to disk)
- [x] B-tree implementation
- [x] Multi-column indexing (username, email)
- [x] Pager for memory management
- [x] Proper SQL parser
- [x] SELECT DISTINCT
- [x] ORDER BY, LIMIT, OFFSET
- [x] GROUP BY with aggregate functions (COUNT, SUM, AVG, MIN, MAX)
- [x] HAVING clause for filtering grouped results
- [x] COUNT(DISTINCT col) for counting unique values
- [x] MIN/MAX on string columns
- [x] Multiple GROUP BY columns
- [x] Multi-table support (JOINs - INNER, LEFT, RIGHT)
- [ ] ORDER BY on aggregate columns
- [ ] Combined row output for JOINs (currently filters left table rows)
- [ ] More SQL commands (CREATE TABLE, DROP TABLE, ALTER TABLE)
- [ ] Subqueries
- [ ] Transactions and ACID properties

## References

- https://cstack.github.io/db_tutorial/parts/part1.html
- https://medium.com/@paolorechia/building-a-database-from-scratch-in-rust-part-1-6dfef2223673