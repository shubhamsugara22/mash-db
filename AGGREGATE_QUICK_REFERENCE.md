# Mash_db Aggregate Functions - Quick Reference

## Supported Aggregate Functions

### COUNT
```
COUNT(*)         -- Count all rows
COUNT(column)    -- Count non-null values in column
```

### SUM
```
SUM(column)      -- Sum of numeric column (currently ID only)
```

### AVG
```
AVG(column)      -- Average of numeric column (currently ID only)
```

### MIN
```
MIN(column)      -- Minimum value (currently ID only)
```

### MAX
```
MAX(column)      -- Maximum value (currently ID only)
```

---

## Usage Examples

### Simple Count
```sql
SELECT COUNT(*) FROM users;
-- Returns total number of records
```

### Count with GROUP BY
```sql
SELECT username, COUNT(*) FROM users GROUP BY username;
-- Returns each unique username with count of records
```

### Multiple Aggregates
```sql
SELECT COUNT(*), SUM(id), AVG(id), MIN(id), MAX(id) FROM users;
-- Returns all aggregates in one row
```

### Aggregates with Conditions
```sql
SELECT username, COUNT(*) FROM users WHERE id > 2 GROUP BY username;
-- WHERE clause applied before grouping
```

### Count Specific Column
```sql
SELECT COUNT(username) FROM users;
-- Counts non-empty username values
```

---

## Syntax Rules

1. **GROUP BY Required for Non-Aggregate Columns**
   - If selecting non-aggregate columns with aggregates, must use GROUP BY
   - Example: `SELECT username, COUNT(*) FROM users GROUP BY username;`

2. **Parentheses Required for Aggregates**
   - Must use parentheses: `COUNT(*)`, `SUM(id)`, not `COUNT *` or `SUM id`

3. **Column Names in Aggregates**
   - For COUNT(column) - use actual column name
   - Supported columns: id, username, email
   - Example: `COUNT(id)`, `COUNT(username)`

4. **WHERE Applied Before Grouping**
   - Filtering happens before GROUP BY operation
   - Example: `... WHERE condition ... GROUP BY ...`

---

## Results Format

Each result is printed as a tuple:
```
(value1, value2, value3)
```

### Example Results:

```
-- COUNT(*) from 3 users
(3)

-- GROUP BY username, COUNT(*)
(alice, 1)
(bob, 1)
(charlie, 1)

-- Multiple aggregates
(3, 6, 2.00)

-- Mixed columns with aggregate
(alice, 2)
(bob, 1)
```

---

## Limitations (Current Version)

- ✗ String aggregation (MIN/MAX on username/email)
- ✗ HAVING clause (filter on aggregate results)
- ✗ ORDER BY on grouped results
- ✗ LIMIT/OFFSET with GROUP BY
- ✗ DISTINCT in aggregates (DISTINCT keyword)
- ✗ Multiple GROUP BY columns (single column only currently)
- ✗ Aggregate functions on expressions

---

## Test Suite Status

✅ 65 Unit Tests Passing
- 18 aggregate function tests
- 47 existing feature tests

---

## Performance Notes

- Grouping uses HashMap for O(1) lookup per group
- Row references used (no copying)
- Single pass over result set
- Suitable for typical database operations (100s-1000s of records)

---

## Error Handling

If aggregate syntax is invalid:
```
SELECT COUNT( FROM users;
-- Error: Expected ( after COUNT

SELECT SUM(*) FROM users;
-- Valid but may return unexpected result (SUM requires column)
```

---

## Future Enhancements

Planned for next releases:
- [ ] HAVING clause support
- [ ] ORDER BY with aggregates
- [ ] LIMIT/OFFSET with GROUP BY
- [ ] String column aggregates
- [ ] DISTINCT in COUNT
- [ ] GROUP BY multiple columns
