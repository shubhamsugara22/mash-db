# Mash DB

A simple database implementation in Rust, built from scratch following SQLite architecture.

## Current Features

- **REPL Interface** - Interactive command-line interface with `db >` prompt
- **Meta Commands** - `.exit` to quit the database, `.save <filename>` to save data, `.load <filename>` to load data
- **Basic SQL Support**:
  - `INSERT INTO table VALUES (id, 'username', 'email')` - Insert a row
  - `SELECT * FROM table` - Retrieve all rows
  - `SELECT * FROM table WHERE column = 'value'` - Select rows with WHERE condition
  - `UPDATE table SET column = 'value' WHERE id = number` - Update a row
  - `DELETE FROM table WHERE id = number` - Delete a row by ID
  - `DELETE FROM table WHERE column = 'value'` - Delete rows with WHERE condition
- **B-Tree Indexing** - Efficient O(log n) lookups for ID-based operations, and fast lookups for username and email
- **In-Memory Storage** - Table stores rows in a Vec with B-Tree indexes
- **Data Validation** - Username (max 32 chars) and email (max 255 chars) length checks
- **Persistence** - Save/load table to/from JSON files

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
db > SELECT * FROM users
(1, alice, alice@example.com)
(2, bob, bob@example.com)
Executed.
db > UPDATE users SET username = 'bobby' WHERE id = 2
Executed.
db > SELECT * FROM users WHERE username = 'bobby'
(2, bobby, bob@example.com)
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
- [ ] More SQL commands (JOINs, advanced WHERE)
- [x] Proper SQL parser

## References

- https://cstack.github.io/db_tutorial/parts/part1.html
- https://medium.com/@paolorechia/building-a-database-from-scratch-in-rust-part-1-6dfef2223673