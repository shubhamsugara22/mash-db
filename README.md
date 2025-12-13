# Mash DB

A simple database implementation in Rust, built from scratch following SQLite architecture.

## Current Features

- **REPL Interface** - Interactive command-line interface with `db >` prompt
- **Meta Commands** - `.exit` to quit the database
- **Basic SQL Support**:
  - `insert <id> <username> <email>` - Insert a row
  - `select` - Retrieve all rows
  - `update <id> set <column>=<value>` - Update a row (e.g., `update 1 set username=alice2`)
- **In-Memory Storage** - Table stores rows in a Vec
- **Data Validation** - Username (max 32 chars) and email (max 255 chars) length checks

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
db > update 1 set username=alice2
Executed.
db > select
(1, alice2, alice@example.com)
(2, bob, bob@example.com)
Executed.
db > .exit
Bye!
```

## Architecture

- `main.rs` - REPL loop, command parsing, and statement execution
- `table.rs` - Row and Table structures for data storage
- `column.rs` - (Reserved for future column definitions)

## TODO

- [ ] Persistence (save to disk)
- [ ] B-tree implementation
- [ ] Pager for memory management
- [ ] More SQL commands (UPDATE, DELETE, WHERE clauses)
- [ ] Proper SQL parser

## References

- https://cstack.github.io/db_tutorial/parts/part1.html
- https://medium.com/@paolorechia/building-a-database-from-scratch-in-rust-part-1-6dfef2223673