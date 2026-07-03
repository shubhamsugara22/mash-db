# Mash DB

A simple database implementation in Rust, built from scratch following SQLite architecture.

## 🎉 Major Feature: Dynamic Schema Support

**NEW** - The database now supports custom table schemas with arbitrary columns! No longer limited to (id, username, email).

### Examples

```sql
-- Create a custom table with any columns
CREATE TABLE products (id, name, price, stock, category)
INSERT INTO products VALUES (1, 'Widget', '19.99', '100', 'Tools')
SELECT * FROM products
-- Output: (1, Widget, 19.99, 100, Tools)

-- Create another table with different schema
CREATE TABLE stores (id, name, city, opening_year, manager)
INSERT INTO stores VALUES (1, 'Downtown', 'NYC', '2020', 'John')
SELECT name, manager FROM stores
-- Output: (Downtown, John)

-- All operations work with custom columns
SELECT category, SUM(stock) FROM products GROUP BY category
SELECT name FROM products WHERE price > '15' ORDER BY name
```

## Current Features

- **Dynamic Schema Support** ✨ NEW - Create tables with any columns (no fixed structure)
- **REPL Interface** - Interactive command-line interface with `db >` prompt
- **Meta Commands** - `.exit` to quit the database, `.save <filename>` to save data, `.load <filename>` to load data
- **Basic SQL Support**:
  - `INSERT INTO table VALUES (val1, val2, val3, ...)` - Insert rows with custom columns
  - `SELECT * FROM table` - Retrieve all rows with all columns
  - `SELECT col1, col2 FROM table` - Select specific columns
  - `SELECT * FROM table WHERE column = 'value'` - Select rows with WHERE condition
  - `SELECT DISTINCT column FROM table` - Select unique values
  - `UPDATE table SET column = 'value' WHERE id = number` - Update a row
  - `DELETE FROM table WHERE id = number` - Delete a row by ID
  - `DELETE FROM table WHERE column = 'value'` - Delete rows with WHERE condition
- **DDL Commands**:
  - `CREATE TABLE table_name (col1, col2, col3, ...)` - Create a new table with custom columns
  - `DROP TABLE table_name` - Drop an existing table (removes from registry and deletes .json file)
  - `ALTER TABLE table_name RENAME TO new_name` - Rename a table
- **Advanced SQL Features**:
  - **Aggregate Functions**: `COUNT(*)`, `COUNT(col)`, `COUNT(DISTINCT col)`, `SUM(col)`, `AVG(col)`, `MIN(col)`, `MAX(col)`
    - ✅ **Works with custom columns**: Aggregates compute over any column
    - ✅ **ORDER BY on aggregates**: Sort grouped results by aggregate values (e.g., `ORDER BY COUNT(*) DESC`)
  - **Scalar Functions**: Over 20 built-in functions for string, numeric, and conditional operations
    - **String**: `UPPER(col)`, `LOWER(col)`, `LENGTH(col)`, `TRIM(col)`, `CONCAT(col1, col2)`, `REPLACE(col, from, to)`, `SUBSTR(col, start, len)`, `LPAD(col, len, pad)`, `RPAD(col, len, pad)`, `LEFT(col, len, pad)`, `RIGHT(col, len)`, `REVERSE(col)`, `REPEAT(col, n)`, `INITCAP(col)`
    - **Numeric**: `ABS(col)`, `ROUND(col, decimals)`, `FLOOR(col)`, `CEIL(col)`
      - **More Numeric**: `SIGN(col)` — returns -1/0/1 for negative/zero/positive values
    - **Conditional**: `IF(col op val, then, else)`, `CASE WHEN ... THEN ... ELSE ... END`, `COALESCE(col, default)`, `NULLIF(col, val)`
    - **Type**: `CAST(col AS type)`
  - **Window Functions**: Analytic functions that operate over partitions of rows
    - `ROW_NUMBER() OVER (PARTITION BY col ORDER BY col2)` - Sequential number within partition
    - `RANK() OVER (ORDER BY col)` - Ranking with gaps for ties
    - `DENSE_RANK() OVER (ORDER BY col)` - Ranking without gaps
    - `LEAD(col, offset, default) OVER (PARTITION BY col2 ORDER BY col3)` - Value from subsequent row
      - `offset` (optional, default 1): Number of rows to look ahead
      - `default` (optional, default NULL): Value when no subsequent row exists
    - `LAG(col, offset, default) OVER (PARTITION BY col2 ORDER BY col3)` - Value from preceding row
      - `offset` (optional, default 1): Number of rows to look back
      - `default` (optional, default NULL): Value when no preceding row exists
  - **Date/Time Functions**: Temporal data manipulation
    - `NOW()` - Current UNIX timestamp
    - `DATE(col)` - Extract date (YYYY-MM-DD) from timestamp or datetime
    - `TIME(col)` - Extract time (HH:MM:SS) from timestamp or datetime
    - `YEAR(col)` - Extract year from date/timestamp
    - `MONTH(col)` - Extract month (1-12) from date/timestamp
    - `DAY(col)` - Extract day (1-31) from date/timestamp
    - `HOUR(col)` - Extract hour (0-23) from time/timestamp
    - `MINUTE(col)` - Extract minute (0-59) from time/timestamp
    - `SECOND(col)` - Extract second (0-59) from time/timestamp
    - `DATE_ADD(col, days)` - Add days to date
    - `DATE_SUB(col, days)` - Subtract days from date
- **Views** ✨ NEW - Create and query database views
  - `CREATE VIEW view_name AS SELECT ...` - Create a view with a SELECT query
  - `DROP VIEW view_name` - Drop an existing view
  - **Features**: Views support WHERE, GROUP BY, ORDER BY, LIMIT, aggregates, joins, and nesting
  - **GROUP BY**: Group results by any columns (e.g., `GROUP BY category, supplier`)
  - **HAVING**: Filter grouped results with conditions
  - **ORDER BY (qualified)**: Sort results ASC/DESC with table qualifiers
  - **LIMIT/OFFSET**: Paginate results
  - **LIKE Operator**: Pattern matching with `%` and `_` wildcards
  - **WHERE with NULL checks**: `IS NULL` and `IS NOT NULL` predicates
  - **Extended Numeric Literals**: Unquoted signed numbers (`-12.5`, `+1`), scientific notation (`1e6`, `-2.5E-3`), and leading-dot decimals (`.5`, `.5e2`)
  - **Table Aliases**: Simplified references using aliases
  - **JOIN Operations**: `INNER`, `LEFT`, `RIGHT` with `ON left.col = right.col`
- **B-Tree Indexing** - Efficient O(log n) lookups for indexed columns
- **In-Memory Storage** - Table stores rows in a Vec with B-Tree indexes for id
- **Data Validation** - Column value length checks
- **Persistence** - Save/load tables to/from JSON files
- **Schema Registry** - Tracks column schemas for each table, persists to schemas.json
- **Backward Compatibility** - Original (id, username, email) fixed schema still fully supported
- **Multiple Table Support** - Different tables can have completely different schemas
- **Comprehensive Testing** - 194 passing tests covering all SQL features and scalar functions

## Usage

```bash
cargo run
```

Example session:

```text
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
db > SELECT users.id, orders.id FROM users INNER JOIN orders ON users.id = orders.id
(1, 1)
(2, 2)
Executed.
db > SELECT users.username, orders.id FROM users LEFT JOIN orders ON users.id = orders.id ORDER BY orders.id ASC LIMIT 2
(alice, 1)
(bob, 2)
Executed.
db > SELECT u.username FROM users u LEFT JOIN orders o ON u.id = o.id WHERE o.id IS NULL
(charlie)
Executed.
db > SELECT username, COUNT(*) FROM orders GROUP BY username ORDER BY COUNT(*) DESC LIMIT 2
(alice, 2)
(bob, 1)
Executed.
db > SELECT id, SUM(id) FROM orders GROUP BY id ORDER BY SUM(id) DESC
(3, 3)
(2, 2)
Executed.
db > SELECT username FROM users WHERE username LIKE 'al%'
(alice)
Executed.
db > INSERT INTO metrics VALUES (1, baseline, -12.5, delta)
Executed.
db > SELECT name, reading FROM metrics WHERE reading >= .5e2
(spike, 1.25e3)
Executed.
db > .exit
Bye!
```

### DDL Commands - CREATE TABLE and DROP TABLE

```sql
-- Create a new table
db > CREATE TABLE products (id, name, price)
Table 'products' created with columns: id, name, price

-- Create another table
db > CREATE TABLE inventory (id, product_id, quantity)
Table 'inventory' created with columns: id, product_id, quantity

-- Drop a table
db > DROP TABLE inventory
Table 'inventory' dropped

-- Try to drop non-existent table (error)
db > DROP TABLE nonexistent
Error: Table 'nonexistent' does not exist
```

## Examples - Advanced Usage

### ORDER BY on Aggregate Functions

```sql
-- Count orders per user, sorted by highest count first
SELECT username, COUNT(*) FROM orders GROUP BY username ORDER BY COUNT(*) DESC

-- Sum amounts per user, sorted by total spending (lowest first)
SELECT username, SUM(amount) FROM orders GROUP BY username ORDER BY SUM(amount) ASC

-- Find users with most distinct order IDs
SELECT username, COUNT(DISTINCT id) FROM orders GROUP BY username ORDER BY COUNT(DISTINCT id) DESC
```

### GROUP BY with LIMIT/OFFSET on Aggregates

```sql
-- Top 3 users by order count
SELECT username, COUNT(*) FROM orders GROUP BY username ORDER BY COUNT(*) DESC LIMIT 3

-- Skip first 2 users, get next 3 by count
SELECT username, COUNT(*) FROM orders GROUP BY username ORDER BY COUNT(*) DESC LIMIT 3 OFFSET 2
```

### Complex Queries Combining All Features

```sql
-- High-value orders for users with multiple orders, sorted by total
SELECT username, COUNT(*), SUM(amount) FROM orders 
WHERE amount > 100 
GROUP BY username 
HAVING COUNT(*) > 1 
ORDER BY SUM(amount) DESC 
LIMIT 5
```

### LIKE Pattern Matching

```sql
-- Starts with 'al'
SELECT username FROM users WHERE username LIKE 'al%'

-- Ends with '@example.com'
SELECT email FROM users WHERE email LIKE '%@example.com'

-- Contains 'li'
SELECT username FROM users WHERE username LIKE '%li%'

-- Single character wildcard
SELECT username FROM users WHERE username LIKE 'b_b'
```

### Scalar Functions

```sql
-- String manipulation
SELECT UPPER(username), LOWER(email) FROM users
SELECT INITCAP(name) FROM products  -- Capitalize first letter of each word
SELECT CONCAT(username, '@', 'domain.com') FROM users
SELECT LEFT(username, 3), RIGHT(email, 10) FROM users
SELECT REVERSE(username), REPEAT(category, 3) FROM products
SELECT REPLACE(email, 'example.com', 'newdomain.com') FROM users
SELECT LPAD(name, 20, '-'), RPAD(category, 15, '.') FROM products

-- Numeric functions
SELECT ABS(balance), ROUND(price, 2) FROM accounts
SELECT FLOOR(price), CEIL(price) FROM products  -- Round down/up
SELECT MOD(quantity, 10), POWER(rating, 2), SQRT(area) FROM inventory
SELECT POSITION('ll', description), INSTR(description, 'el'), SUBSTRING_INDEX(description, ',', 2) FROM products
SELECT SUBSTR(description, 1, 50) FROM products

-- Conditional logic
SELECT IF(stock > 100, 'High', 'Low') FROM products
SELECT COALESCE(middle_name, 'N/A'), NULLIF(status, 'inactive') FROM users
SELECT CASE WHEN price > 100 THEN 'Premium' WHEN price > 50 THEN 'Standard' ELSE 'Budget' END FROM products

-- Type conversion
SELECT CAST(price AS INTEGER), CAST(id AS TEXT) FROM products

-- Date/Time functions
SELECT NOW() FROM users  -- Current UNIX timestamp
SELECT DATE(created_at), TIME(created_at) FROM events
SELECT YEAR(birthdate), MONTH(birthdate), DAY(birthdate) FROM users
SELECT HOUR(created_at), MINUTE(created_at), SECOND(created_at) FROM events
SELECT * FROM orders WHERE YEAR(order_date) = 2024
SELECT MONTH(purchase_date), COUNT(*) FROM sales GROUP BY MONTH(purchase_date)
SELECT HOUR(event_time), COUNT(*) FROM events GROUP BY HOUR(event_time)
SELECT DATE_ADD(order_date, 30) as delivery_date FROM orders
SELECT DATE_SUB(TODAY, 90) as three_months_ago

-- Common Table Expressions (CTEs with WITH clause)
WITH high_value_orders AS (
    SELECT id, amount FROM orders WHERE amount > 1000
)
SELECT * FROM high_value_orders

WITH monthly_sales AS (
    SELECT MONTH(sale_date) as month, SUM(amount) as total
    FROM sales
    GROUP BY MONTH(sale_date)
)
SELECT month, total FROM monthly_sales ORDER BY total DESC

WITH recent_users AS (
    SELECT id, username FROM users WHERE YEAR(created_at) = 2024
)
SELECT username FROM recent_users ORDER BY username

-- Index creation and management
CREATE INDEX idx_email ON users (email)                    -- Create index on email
CREATE INDEX idx_order_date ON orders (order_date)        -- Create index on date column
CREATE INDEX idx_customer_id ON orders (customer_id)      -- Create index for JOINs
DROP INDEX idx_order_date                                 -- Drop unused index

-- Views for simplified queries
CREATE VIEW user_emails AS SELECT id, username, email FROM users
SELECT * FROM user_emails WHERE id > 2
SELECT * FROM user_emails ORDER BY username

-- Views with aggregation
CREATE VIEW order_summary AS SELECT username, COUNT(*) as total FROM orders GROUP BY username
SELECT * FROM order_summary WHERE total > 1

-- Nested views
CREATE VIEW active_orders AS SELECT * FROM orders WHERE id > 0
CREATE VIEW active_order_summary AS SELECT COUNT(*) FROM active_orders
SELECT * FROM active_order_summary

-- Drop views
DROP VIEW user_emails
DROP VIEW order_summary
DROP VIEW active_orders
DROP VIEW active_order_summary
```

## DDL Commands - Index Management

```sql
-- Create indexes for performance optimization
db > CREATE INDEX idx_email ON users (email)
Index 'idx_email' created on users.email

-- Create multiple indexes for different purposes
db > CREATE INDEX idx_price ON products (price)
Index 'idx_price' created on products.price

db > CREATE INDEX idx_customer ON orders (customer_id)
Index 'idx_customer' created on orders.customer_id

-- Drop an index
db > DROP INDEX idx_email
Index 'idx_email' dropped

-- Try to drop non-existent index (error)
db > DROP INDEX nonexistent
Error: Index 'nonexistent' not found
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
- [x] ORDER BY on aggregate columns
- [x] Combined row output for JOINs
- [x] Table alias support (e.g., `users u`, `orders o`)
- [x] WHERE `IS NULL` / `IS NOT NULL`
- [x] CREATE TABLE and DROP TABLE commands
- [x] LIKE operator for pattern matching
- [x] Extended Numeric Literals (signed, scientific notation, leading-dot decimals)
- [x] Scalar Functions (20+ functions)
  - [x] String functions: UPPER, LOWER, LENGTH, TRIM, CONCAT, REPLACE, SUBSTR, LPAD, RPAD, LEFT, RIGHT, REVERSE, REPEAT, INITCAP, POSITION, INSTR, SUBSTRING_INDEX
  - [x] Numeric functions: ABS, ROUND, FLOOR, CEIL, MOD, POWER, SQRT
  - [x] Conditional functions: IF, CASE/WHEN/THEN/ELSE/END, COALESCE, NULLIF
  - [x] Type conversion: CAST
- [x] ALTER TABLE (add/drop columns, rename) — metadata only
- [x] Dynamic Schema Support (custom table columns)
- [x] SHOW TABLES command
- [x] TRUNCATE TABLE command
- [x] Subqueries — IN subquery support
- [x] Transactions — BEGIN/COMMIT/ROLLBACK (snapshot-based)
- [x] BETWEEN operator for range queries
- [x] Window functions (ROW_NUMBER, RANK, DENSE_RANK, LEAD, LAG with offset/default) — PARTITION BY and ORDER BY supported
- [x] Date/Time functions (NOW, DATE, TIME, YEAR, MONTH, DAY)
- [x] Common Table Expressions (WITH clause) — Parser complete, execution integration pending
- [x] CREATE INDEX / DROP INDEX statements — Parser complete for explicit index creation
- [x] Date/Time functions (NOW, DATE, TIME, YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, DATE_ADD, DATE_SUB)
- [x] Views (CREATE VIEW, DROP VIEW) — Views with subquery substitution, support for aggregates, joins, nested views
- [ ] More date functions (DATEDIFF, DATE_TRUNC, WEEK, QUARTER)
- [ ] Full text search
- [ ] Foreign key constraints
- [ ] PRIMARY KEY and UNIQUE constraints
- [ ] CREATE INDEX on arbitrary columns

## References

- <https://cstack.github.io/db_tutorial/parts/part1.html>
- <https://medium.com/@paolorechia/building-a-database-from-scratch-in-rust-part-1-6dfef2223673>
