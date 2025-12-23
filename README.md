# Mash DB

A simple database implementation in Rust, built from scratch following SQLite architecture.

## Current Features

- **REPL Interface** - Interactive command-line interface with `db >` prompt
- **Meta Commands** - `.exit` to quit the database, `.save <filename>` to save data, `.load <filename>` to load data
- **Basic SQL Support**:
  - `insert <id> <username> <email>` - Insert a row
  - `select` - Retrieve all rows
  - `select where <column>=<value>` - Select rows with WHERE condition (e.g., `select where id=1`)
  - `update <id> set <column>=<value>` - Update a row (e.g., `update 1 set username=alice2`)
  - `delete <id>` - Delete a row by ID
  - `delete where <column>=<value>` - Delete rows with WHERE condition
- **B-Tree Indexing** - Efficient O(log n) lookups for ID-based operations
- **In-Memory Storage** - Table stores rows in a Vec with B-Tree index
- **Data Validation** - Username (max 32 chars) and email (max 255 chars) length checks
- **Persistence** - Save/load table to/from JSON files

## Usage

```bash
cargo run
```

Example session:
```
db > insert 1 alice alice@example.com
Executed.
db > insert 2 bob bob@example.com
Executed.
db > .save mydata.json
Saved to 'mydata.json'.
db > .exit
Bye!
```

Then restart:
```
db > .load mydata.json
Loaded from 'mydata.json'.
db > select
(1, alice, alice@example.com)
(2, bob, bob@example.com)
Executed.
db > .exit
Bye!
```

## Architecture

- `main.rs` - REPL loop, command parsing, and statement execution
- `table.rs` - Row and Table structures for data storage with B-Tree indexing
- `column.rs` - (Reserved for future column definitions)

## TODO

- [x] Persistence (save to disk)
- [x] B-tree implementation
- [ ] Pager for memory management
- [ ] More SQL commands (JOINs, advanced WHERE)
- [ ] Proper SQL parser

## References

- https://cstack.github.io/db_tutorial/parts/part1.html
- https://medium.com/@paolorechia/building-a-database-from-scratch-in-rust-part-1-6dfef2223673