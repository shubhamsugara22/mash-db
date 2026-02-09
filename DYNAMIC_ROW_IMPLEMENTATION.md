# Dynamic Row Structure Implementation - Change Summary

## Overview
This document outlines the comprehensive changes made to implement dynamic row structure in Mash DB, allowing tables to have custom columns beyond the hardcoded id/username/email fields.

## Key Changes

### 1. **table.rs** - Core Row Structure
- **Row struct**: Added `extras: HashMap<String, String>` field to store dynamic columns
- **Row::from_values()**: New constructor that creates rows from schema and values
- **Row::get_value()** and **Row::get_value_ref()**: Accessors for any column (fixed or dynamic)
- **Table struct**: Added `schema: Vec<String>` and `has_id`, `has_username`, `has_email` flags
- **Table::new()**: Now requires schema parameter
- **Table::set_schema()**: Allows schema updates
- **Conditional indexing**: Only builds indexes for columns that exist in schema
- **Generic column operations**: UPDATE, DELETE, SELECT now work with any column in schema

### 2. **parser.rs** - INSERT Parsing
- **parse_insert()**: Changed return type from `(Option<String>, u32, String, String)` to `(Option<String>, Vec<String>)`
- Now parses variable-length value lists: `INSERT INTO table VALUES (val1, val2, val3, ...)`
- Compatible with simple format: `INSERT id username email ...`

### 3. **main.rs** - Statement Execution

#### Statement::Insert
- Changed from fixed fields `{id, username, email}` to `{values: Vec<String>}`
- Execution now uses schema-aware row construction

#### Functions Updated
- **group_rows_by_columns()**: Added `schema` parameter, uses `row.get_value()` for any column
- **compute_aggregate()**: Added `schema` parameter, works with any numeric/string column
- **load_table_by_name()**: Now fetches schema from `schemas` HashMap
- **get_default_table()**: Passes schemas to `load_table_by_name`
- **execute_subquery_for_in()**: Added `schemas` parameter
- **resolve_in_subqueries()**: Added `schemas` parameter
- All aggregate and GROUP BY operations now schema-aware

#### TODO: Remaining Changes Needed
The following changes still need to be made in `main.rs`:

1. **Statement::Insert execution** (line ~1037-1045):
```rust
// OLD:
Statement::Insert {
    table_name,
    id,
    username,
    email,
} => {
    let table = if let Some(name) = table_name.as_deref() {
        load_table_by_name(name, tables)
    } else {
        get_default_table(tables)
    };
    match Row::new(id, username, email) {
        Ok(row) => match table.insert(row) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Err(e) => println!("Error: {}", e),
    }
}

// NEW:
Statement::Insert {
    table_name,
    values,
} => {
    let table = if let Some(name) = table_name.as_deref() {
        load_table_by_name(name, tables, schemas)
    } else {
        get_default_table(tables, schemas)
    };
    let table_schema = table.schema().clone();
    match Row::from_values(&table_schema, values) {
        Ok(row) => match table.insert(row) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Err(e) => println!("Error: {}", e),
    }
}
```

2. **SELECT statement execution**: Replace `load_table_by_name(table_name, tables)` with `load_table_by_name(table_name, tables, schemas)` (multiple locations)

3. **JOIN operations**: Update `apply_join()` to use `row.get_value()` instead of hardcoded field access

4. **Print output**: Update all `println!()` statements that print rows to iterate over schema columns dynamically

5. **All aggregate calls**: Add schema parameter: `compute_aggregate(agg, &rows, &table_schema)`

6. **All group_by calls**: Add schema parameter: `group_rows_by_columns(rows, group_cols, &table_schema)`

7. **CREATE TABLE execution**: Set schema on newly created tables:
```rust
Statement::CreateTable {
    table_name,
    columns,
} => {
    let table_name_lower = table_name.to_lowercase();
    let file_path = table_file_for(&table_name_lower);
    
    // Create table with schema
    let new_table = Table::new(file_path, columns.clone());
    tables.insert(table_name_lower.clone(), new_table);
    
    // Save schema
    schemas.insert(table_name_lower, columns.clone());
    if !tx.active {
        save_schemas(schemas);
    }
    
    println!("Table '{}' created with columns: {}", table_name, columns.join(", "));
}
```

8. **DROP TABLE**: Already correct (just removes from schemas HashMap)

9. **ALTER TABLE ADD COLUMN**: Update table schema:
```rust
Statement::AlterTableAddColumn {
    table_name,
    column,
} => {
    let table_name_lower = table_name.to_lowercase();
    
    if let Some(schema) = schemas.get_mut(&table_name_lower) {
        if schema.contains(&column) {
            println!("Error: Column '{}' already exists", column);
            return;
        }
        schema.push(column.clone());
        
        // Update table schema
        if let Some(table) = tables.get_mut(&table_name_lower) {
            table.set_schema(schema.clone());
        }
        
        if !tx.active {
            save_schemas(schemas);
        }
        
        println!("Column '{}' added to table '{}'", column, table_name);
    } else {
        println!("Error: Table '{}' does not exist", table_name);
    }
}
```

10. **ALTER TABLE DROP COLUMN**: Update table schema:
```rust
Statement::AlterTableDropColumn {
    table_name,
    column,
} => {
    let table_name_lower = table_name.to_lowercase();
    
    if let Some(schema) = schemas.get_mut(&table_name_lower) {
        schema.retain(|c| c != &column);
        
        // Update table schema
        if let Some(table) = tables.get_mut(&table_name_lower) {
            table.set_schema(schema.clone());
            // Note: Existing row data in extras will still have the old column
            // Physical row modification not yet implemented
        }
        
        if !tx.active {
            save_schemas(schemas);
        }
        
        println!("Column '{}' dropped from table '{}' (metadata only)", column, table_name);
    } else {
        println!("Error: Table '{}' does not exist", table_name);
    }
}
```

11. **UPDATE output for custom columns**:
```rust
for row in rows {
    match &columns {
        None => {
            // SELECT * - print all schema columns
            let mut values = Vec::new();
            for col in table_schema.iter() {
                values.push(row.get_value(col).unwrap_or("NULL".to_string()));
            }
            println!("({})", values.join(", "));
        }
        Some(cols) => {
            let mut values: Vec<String> = Vec::new();
            for col in cols.iter() {
                let col_name = extract_column_name(col);
                values.push(row.get_value(col_name).unwrap_or(format!("NULL({})", col_name)));
            }
            println!("({})", values.join(", "));
        }
    }
}
```

12. **apply_sorting()**: Make schema-aware:
```rust
fn apply_sorting(mut rows: Vec<&Row>, order_by: Option<(String, bool)>, schema: &[String]) -> Vec<&Row> {
    if let Some((column, is_asc)) = order_by {
        let col_name: &str = if let Some(idx) = column.rfind('.') {
            &column[idx + 1..]
        } else {
            &column
        };
        if !schema.iter().any(|c| c == col_name) {
            return rows; // Column doesn't exist, skip sorting
        }
        rows.sort_by(|a, b| {
            let val_a = a.get_value(col_name);
            let val_b = b.get_value(col_name);
            let cmp = match (val_a, val_b) {
                (Some(va), Some(vb)) => {
                    // Try numeric comparison first
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        va.cmp(&vb)
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            };
            if is_asc {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }
    rows
}
```

13. **apply_distinct()**: Use schema for tuple creation:
```rust
fn apply_distinct(rows: Vec<&Row>, distinct: bool, schema: &[String]) -> Vec<&Row> {
    if !distinct {
        return rows;
    }

    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut unique_rows = Vec::new();

    for row in rows {
        let mut row_tuple = Vec::new();
        for col in schema.iter() {
            row_tuple.push(row.get_value(col).unwrap_or("NULL".to_string()));
        }
        let key = row_tuple.join("|");
        if seen.insert(key) {
            unique_rows.push(row);
        }
    }

    unique_rows
}
```

14. **main() initialization**:
```rust
fn main() {
    // Initialize table registry with default "users" table
    let mut tables: HashMap<String, Table> = HashMap::new();
    let default_schema = vec![
        "id".to_string(),
        "username".to_string(),
        "email".to_string(),
    ];
    tables.insert("users".to_string(), Table::new("data.json".to_string(), default_schema.clone()));

    // Load or initialize schema registry
    let mut schemas = load_schemas();
    if schemas.is_empty() {
        schemas.insert("users".to_string(), default_schema.clone());
        schemas.insert("orders".to_string(), default_schema.clone());
        save_schemas(&schemas);
    }

    // ... rest of initialization
}
```

## Testing Requirements

1. **CREATE TABLE with custom columns**:
```sql
CREATE TABLE products (id, name, price, stock)
INSERT INTO products VALUES (1, 'Widget', '19.99', '100')
SELECT * FROM products
```

2. **ALTER TABLE ADD/DROP COLUMN**:
```sql
ALTER TABLE products ADD COLUMN category
ALTER TABLE products DROP COLUMN stock
```

3. **Queries on custom columns**:
```sql
SELECT name, price FROM products WHERE price > '10'
SELECT name, SUM(price) FROM products GROUP BY name
```

4. **JOINs with custom columns**:
```sql
CREATE TABLE orders (id, product_id, quantity, total)
SELECT products.name, orders.quantity FROM products INNER JOIN orders ON products.id = orders.product_id
```

## Benefits

1. **True multi-table support**: Each table can have different columns
2. **Schema flexibility**: Add/remove columns via ALTER TABLE
3. **Type-agnostic operations**: All values stored as strings, operations infer types
4. **Backward compatibility**: Existing id/username/email tables still work
5. **Extensible**: Foundation for future type system and constraints

## Limitations

1. **Type inference only**: All values stored as strings, types inferred at query time
2. **No NULL enforcement**: Cannot mark columns as NOT NULL
3. **Extras overhead**: Dynamic columns stored in HashMap (slower than fixed fields)
4. **Physical ALTER incomplete**: ADD/DROP COLUMN don't modify existing row data
5. **Index limitations**: Only id/username/email are indexed (custom columns use sequential scan)

## Next Steps

1. Complete all TODO items in main.rs
2. Update all tests to pass schemas parameter
3. Add comprehensive tests for custom columns
4. Document breaking changes in README
5. Consider adding custom column indexing
6. Implement physical ALTER TABLE operations
7. Add proper data type system (Phase 2)
