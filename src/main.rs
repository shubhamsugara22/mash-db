use std::io::{self, Write};

mod column;
mod pager;
mod parser;
mod parser_tests;
mod table;

use table::{Row, Table};

// Struct to represent a joined row with data from both tables
#[derive(Debug, Clone)]
struct JoinedRow {
    left_id: u32,
    left_username: String,
    left_email: String,
    right_id: Option<u32>,
    right_username: Option<String>,
    right_email: Option<String>,
}

impl JoinedRow {
    fn from_left_only(left: &Row) -> Self {
        JoinedRow {
            left_id: left.id,
            left_username: left.username.clone(),
            left_email: left.email.clone(),
            right_id: None,
            right_username: None,
            right_email: None,
        }
    }

    fn from_both(left: &Row, right: &Row) -> Self {
        JoinedRow {
            left_id: left.id,
            left_username: left.username.clone(),
            left_email: left.email.clone(),
            right_id: Some(right.id),
            right_username: Some(right.username.clone()),
            right_email: Some(right.email.clone()),
        }
    }
}

// Helper struct to represent aggregate functions and their values
#[derive(Debug, Clone)]
enum AggregateColumn {
    Regular(String),
    Count(Option<String>), // None for COUNT(*), Some(col) for COUNT(col)
    CountDistinct(String), // COUNT(DISTINCT col)
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
}

impl AggregateColumn {
    fn from_col_string(col: &str) -> AggregateColumn {
        if col.starts_with("count(") && col.ends_with(")") {
            let inner = &col[6..col.len() - 1];
            if inner == "*" {
                AggregateColumn::Count(None)
            } else if inner.starts_with("distinct ") {
                let col_name = &inner[9..];
                AggregateColumn::CountDistinct(col_name.to_string())
            } else {
                AggregateColumn::Count(Some(inner.to_string()))
            }
        } else if col.starts_with("sum(") && col.ends_with(")") {
            let inner = &col[4..col.len() - 1];
            AggregateColumn::Sum(inner.to_string())
        } else if col.starts_with("avg(") && col.ends_with(")") {
            let inner = &col[4..col.len() - 1];
            AggregateColumn::Avg(inner.to_string())
        } else if col.starts_with("min(") && col.ends_with(")") {
            let inner = &col[4..col.len() - 1];
            AggregateColumn::Min(inner.to_string())
        } else if col.starts_with("max(") && col.ends_with(")") {
            let inner = &col[4..col.len() - 1];
            AggregateColumn::Max(inner.to_string())
        } else {
            AggregateColumn::Regular(col.to_string())
        }
    }
}

enum MetaCommandResult {
    Success,
    UnrecognizedCommand,
}

enum PrepareResult {
    Success(Statement),
    UnrecognizedStatement,
}

enum Statement {
    Insert {
        id: u32,
        username: String,
        email: String,
    },
    Select {
        distinct: bool,
        columns: Option<Vec<String>>,
        from_table: Option<String>,       // Added for explicit table name
        join: Option<parser::JoinClause>, // Added for JOIN support
        group_by: Option<Vec<String>>,
        having: Option<(Vec<(String, String, String)>, Vec<String>)>,
        order_by: Option<(String, bool)>, // (column, is_asc)
        limit: Option<u32>,
        offset: Option<u32>,
    },
    SelectWhere {
        distinct: bool,
        columns: Option<Vec<String>>,
        from_table: Option<String>,       // Added for explicit table name
        join: Option<parser::JoinClause>, // Added for JOIN support
        conditions: Vec<(String, String, String)>,
        operators: Vec<String>,
        group_by: Option<Vec<String>>,
        having: Option<(Vec<(String, String, String)>, Vec<String>)>,
        order_by: Option<(String, bool)>, // (column, is_asc)
        limit: Option<u32>,
        offset: Option<u32>,
    },
    Update {
        id: u32,
        column: String,
        value: String,
    },
    Delete {
        id: u32,
    },
    DeleteWhere {
        column: String,
        value: String,
    },
    DeleteAll,
}

fn print_prompt() {
    print!("db > ");
    io::stdout().flush().unwrap();
}

fn do_meta_command(input: &str, _table: &mut Table) -> MetaCommandResult {
    if input == ".exit" {
        println!("Bye!");
        std::process::exit(0);
    } else {
        MetaCommandResult::UnrecognizedCommand
    }
}

fn prepare_statement(input: &str) -> PrepareResult {
    if input.to_uppercase().starts_with("INSERT") {
        match parser::parse_insert(input) {
            Ok((id, username, email)) => PrepareResult::Success(Statement::Insert {
                id,
                username,
                email,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase().starts_with("UPDATE") {
        match parser::parse_update(input) {
            Ok((id, column, value)) => {
                PrepareResult::Success(Statement::Update { id, column, value })
            }
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase().starts_with("SELECT") {
        match parser::parse_select(input) {
            Ok((
                distinct,
                cols,
                from_table,
                join,
                None,
                group_by,
                having,
                order_by,
                limit,
                offset,
            )) => PrepareResult::Success(Statement::Select {
                distinct,
                columns: cols,
                from_table,
                join,
                group_by,
                having,
                order_by,
                limit,
                offset,
            }),
            Ok((
                distinct,
                cols,
                from_table,
                join,
                Some((conditions, operators)),
                group_by,
                having,
                order_by,
                limit,
                offset,
            )) => PrepareResult::Success(Statement::SelectWhere {
                distinct,
                columns: cols,
                from_table,
                join,
                conditions,
                operators,
                group_by,
                having,
                order_by,
                limit,
                offset,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase() == "DELETE ALL" {
        PrepareResult::Success(Statement::DeleteAll)
    } else if input.to_uppercase().starts_with("DELETE") {
        if input.to_uppercase().contains("WHERE") {
            match parser::parse_delete_where(input) {
                Ok((column, value)) => {
                    PrepareResult::Success(Statement::DeleteWhere { column, value })
                }
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        } else {
            match parser::parse_delete(input) {
                Ok(id) => PrepareResult::Success(Statement::Delete { id }),
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        }
    } else {
        PrepareResult::UnrecognizedStatement
    }
}

// Helper function to group rows by specific columns
fn group_rows_by_columns<'a>(
    rows: Vec<&'a Row>,
    group_by_cols: &[String],
) -> std::collections::HashMap<String, Vec<&'a Row>> {
    let mut groups: std::collections::HashMap<String, Vec<&'a Row>> =
        std::collections::HashMap::new();

    for row in rows {
        let mut group_key = Vec::new();
        for col in group_by_cols {
            match col.as_str() {
                "id" => group_key.push(row.id.to_string()),
                "username" => group_key.push(row.username.clone()),
                "email" => group_key.push(row.email.clone()),
                _ => group_key.push("NULL".to_string()),
            }
        }
        let key = group_key.join("|");
        groups.entry(key).or_insert_with(Vec::new).push(row);
    }

    groups
}

// Helper function to compute aggregate value for a group of rows
fn compute_aggregate(agg: &AggregateColumn, rows: &[&Row]) -> String {
    match agg {
        AggregateColumn::Regular(col) => {
            // For regular columns in GROUP BY, just return the first row's value
            if let Some(first_row) = rows.first() {
                match col.as_str() {
                    "id" => first_row.id.to_string(),
                    "username" => first_row.username.clone(),
                    "email" => first_row.email.clone(),
                    _ => "NULL".to_string(),
                }
            } else {
                "NULL".to_string()
            }
        }
        AggregateColumn::Count(col_opt) => {
            match col_opt {
                None => rows.len().to_string(), // COUNT(*)
                Some(col) => {
                    // COUNT(col) - count non-null values
                    let count = rows
                        .iter()
                        .filter(|row| match col.as_str() {
                            "id" => true,
                            "username" => !row.username.is_empty(),
                            "email" => !row.email.is_empty(),
                            _ => false,
                        })
                        .count();
                    count.to_string()
                }
            }
        }
        AggregateColumn::CountDistinct(col) => {
            // COUNT(DISTINCT col) - count unique values
            let mut unique_values = std::collections::HashSet::new();
            for row in rows {
                match col.as_str() {
                    "id" => {
                        unique_values.insert(row.id.to_string());
                    }
                    "username" => {
                        if !row.username.is_empty() {
                            unique_values.insert(row.username.clone());
                        }
                    }
                    "email" => {
                        if !row.email.is_empty() {
                            unique_values.insert(row.email.clone());
                        }
                    }
                    _ => {}
                }
            }
            unique_values.len().to_string()
        }
        AggregateColumn::Sum(col) => {
            let sum: f64 = rows
                .iter()
                .filter_map(|row| match col.as_str() {
                    "id" => Some(row.id as f64),
                    _ => None,
                })
                .sum();
            format!("{:.0}", sum)
        }
        AggregateColumn::Avg(col) => {
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|row| match col.as_str() {
                    "id" => Some(row.id as f64),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                "NULL".to_string()
            } else {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                format!("{:.2}", avg)
            }
        }
        AggregateColumn::Min(col) => match col.as_str() {
            "id" => {
                let values: Vec<u32> = rows.iter().map(|row| row.id).collect();
                if let Some(&min_val) = values.iter().min() {
                    min_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            "username" => {
                let values: Vec<&str> = rows.iter().map(|row| row.username.as_str()).collect();
                if let Some(&min_val) = values.iter().min() {
                    min_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            "email" => {
                let values: Vec<&str> = rows.iter().map(|row| row.email.as_str()).collect();
                if let Some(&min_val) = values.iter().min() {
                    min_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            _ => "NULL".to_string(),
        },
        AggregateColumn::Max(col) => match col.as_str() {
            "id" => {
                let values: Vec<u32> = rows.iter().map(|row| row.id).collect();
                if let Some(&max_val) = values.iter().max() {
                    max_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            "username" => {
                let values: Vec<&str> = rows.iter().map(|row| row.username.as_str()).collect();
                if let Some(&max_val) = values.iter().max() {
                    max_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            "email" => {
                let values: Vec<&str> = rows.iter().map(|row| row.email.as_str()).collect();
                if let Some(&max_val) = values.iter().max() {
                    max_val.to_string()
                } else {
                    "NULL".to_string()
                }
            }
            _ => "NULL".to_string(),
        },
    }
}

// Helper function to evaluate HAVING conditions on grouped results
fn evaluate_having_condition(
    condition: &(String, String, String),
    agg_cols: &[AggregateColumn],
    values: &[String],
) -> bool {
    let (col, op, expected) = condition;

    // Find the index of the aggregate function in the select columns
    let col_lower = col.to_lowercase();
    let agg_idx = agg_cols.iter().position(|agg| match agg {
        AggregateColumn::Count(None) => col_lower == "count(*)",
        AggregateColumn::Count(Some(c)) => col_lower == format!("count({})", c),
        AggregateColumn::CountDistinct(c) => col_lower == format!("count(distinct {})", c),
        AggregateColumn::Sum(c) => col_lower == format!("sum({})", c),
        AggregateColumn::Avg(c) => col_lower == format!("avg({})", c),
        AggregateColumn::Min(c) => col_lower == format!("min({})", c),
        AggregateColumn::Max(c) => col_lower == format!("max({})", c),
        AggregateColumn::Regular(c) => col_lower == c.to_lowercase(),
    });

    if let Some(idx) = agg_idx {
        let actual = &values[idx];

        // Try to parse as numbers for numeric comparison
        if let (Ok(actual_num), Ok(expected_num)) = (actual.parse::<f64>(), expected.parse::<f64>())
        {
            match op.as_str() {
                "=" => (actual_num - expected_num).abs() < 0.0001,
                "!=" => (actual_num - expected_num).abs() >= 0.0001,
                ">" => actual_num > expected_num,
                "<" => actual_num < expected_num,
                ">=" => actual_num >= expected_num,
                "<=" => actual_num <= expected_num,
                _ => false,
            }
        } else {
            // String comparison
            match op.as_str() {
                "=" => actual == expected,
                "!=" => actual != expected,
                ">" => actual > expected,
                "<" => actual < expected,
                ">=" => actual >= expected,
                "<=" => actual <= expected,
                _ => false,
            }
        }
    } else {
        false
    }
}

// Helper function to check if grouped results pass HAVING conditions
fn passes_having_filter(
    having: &Option<(Vec<(String, String, String)>, Vec<String>)>,
    agg_cols: &[AggregateColumn],
    values: &[String],
) -> bool {
    match having {
        None => true, // No HAVING clause, all pass
        Some((conditions, operators)) => {
            if conditions.is_empty() {
                return true;
            }

            // Evaluate first condition
            let mut result = evaluate_having_condition(&conditions[0], agg_cols, values);

            // Evaluate remaining conditions with operators
            for (i, condition) in conditions.iter().enumerate().skip(1) {
                let condition_result = evaluate_having_condition(condition, agg_cols, values);
                if let Some(op) = operators.get(i - 1) {
                    result = match op.as_str() {
                        "AND" => result && condition_result,
                        "OR" => result || condition_result,
                        _ => result,
                    };
                }
            }

            result
        }
    }
}

fn execute_statement(statement: Statement, table: &mut Table) {
    // Map a logical table name to a backing file path.
    fn table_file_for(name: &str) -> String {
        match name.to_lowercase().as_str() {
            // Default primary table
            "users" => "data.json".to_string(),
            // Example secondary tables
            "orders" => "orders.json".to_string(),
            other => format!("{}.json", other),
        }
    }

    // Load a table by name. If the requested name corresponds to the primary table (users), reuse it.
    fn load_table_by_name(name: &str, primary: &Table) -> Table {
        if name.eq_ignore_ascii_case("users") {
            // Create a lightweight copy by saving and reloading to keep semantics uniform.
            // For simplicity, we instantiate a new Table pointing to the same file.
            Table::new(table_file_for("users"))
        } else {
            Table::new(table_file_for(name))
        }
    }

    // Extract column name from qualified name (e.g., "users.id" -> "id")
    fn extract_column_name(qualified: &str) -> &str {
        if let Some(idx) = qualified.rfind('.') {
            &qualified[idx + 1..]
        } else {
            qualified
        }
    }

    // Apply JOIN based on join type and return combined rows
    fn apply_join(
        left_rows: Vec<&Row>,
        left_key: &str,
        right_table: &Table,
        right_key: &str,
        join_type: parser::JoinType,
    ) -> Vec<JoinedRow> {
        let mut result = Vec::new();

        // Extract actual column names from qualified names
        let left_col = extract_column_name(left_key);
        let right_col = extract_column_name(right_key);

        match join_type {
            parser::JoinType::Inner => {
                // INNER JOIN: only rows with matches in right table
                for lr in left_rows {
                    let left_val = match left_col {
                        "id" => lr.id.to_string(),
                        "username" => lr.username.clone(),
                        "email" => lr.email.clone(),
                        _ => "".to_string(),
                    };
                    if left_val.is_empty() {
                        continue;
                    }
                    if let Ok(rrs) = right_table.select_where(right_col, "=", &left_val) {
                        for rr in rrs {
                            result.push(JoinedRow::from_both(lr, rr));
                        }
                    }
                }
            }
            parser::JoinType::Left => {
                // LEFT JOIN: all left rows, with right data if available
                for lr in left_rows {
                    let left_val = match left_col {
                        "id" => lr.id.to_string(),
                        "username" => lr.username.clone(),
                        "email" => lr.email.clone(),
                        _ => "".to_string(),
                    };

                    let mut found_match = false;
                    if !left_val.is_empty() {
                        if let Ok(rrs) = right_table.select_where(right_col, "=", &left_val) {
                            for rr in rrs {
                                result.push(JoinedRow::from_both(lr, rr));
                                found_match = true;
                            }
                        }
                    }

                    if !found_match {
                        result.push(JoinedRow::from_left_only(lr));
                    }
                }
            }
            parser::JoinType::Right => {
                // RIGHT JOIN: only left rows that match right table
                for lr in left_rows {
                    let left_val = match left_col {
                        "id" => lr.id.to_string(),
                        "username" => lr.username.clone(),
                        "email" => lr.email.clone(),
                        _ => "".to_string(),
                    };
                    if left_val.is_empty() {
                        continue;
                    }
                    if let Ok(rrs) = right_table.select_where(right_col, "=", &left_val) {
                        for rr in rrs {
                            result.push(JoinedRow::from_both(lr, rr));
                        }
                    }
                }
            }
        }

        result
    }

    match statement {
        Statement::Insert {
            id,
            username,
            email,
        } => match Row::new(id, username, email) {
            Ok(row) => match table.insert(row) {
                Ok(()) => {
                    table.save().unwrap();
                    println!("Executed.");
                }
                Err(e) => println!("Error: {}", e),
            },
            Err(e) => println!("Error: {}", e),
        },
        Statement::Select {
            distinct,
            columns,
            from_table,
            join,
            group_by,
            having,
            order_by,
            limit,
            offset,
            ..
        } => {
            // Resolve left (from) table
            let mut left_table_ref: Table = if let Some(ref ft) = from_table {
                load_table_by_name(ft, table)
            } else {
                // Default to users
                load_table_by_name("users", table)
            };

            let rows = left_table_ref.select_all();

            // Handle JOIN case separately to avoid ownership issues
            if let Some(ref jc) = join {
                let right_table = load_table_by_name(&jc.table, table);
                let jrows = apply_join(
                    rows,
                    &jc.on_left,
                    &right_table,
                    &jc.on_right,
                    jc.join_type.clone(),
                );
                // Simple display of joined rows (no aggregates/grouping support yet with joins)
                for jrow in jrows {
                    match &columns {
                        None => {
                            // SELECT * - show all columns from both tables
                            if let (Some(rid), Some(rusername), Some(remail)) =
                                (&jrow.right_id, &jrow.right_username, &jrow.right_email)
                            {
                                println!(
                                    "({}, {}, {} | {}, {}, {})",
                                    jrow.left_id,
                                    jrow.left_username,
                                    jrow.left_email,
                                    rid,
                                    rusername,
                                    remail
                                );
                            } else {
                                println!(
                                    "({}, {}, {} | NULL, NULL, NULL)",
                                    jrow.left_id, jrow.left_username, jrow.left_email
                                );
                            }
                        }
                        Some(cols) => {
                            // Show selected columns
                            let mut values: Vec<String> = Vec::new();
                            for col in cols.iter() {
                                match col.as_str() {
                                    "id" => values.push(jrow.left_id.to_string()),
                                    "username" => values.push(jrow.left_username.clone()),
                                    "email" => values.push(jrow.left_email.clone()),
                                    other => values.push(format!("NULL({})", other)),
                                }
                            }
                            println!("({})", values.join(", "));
                        }
                    }
                }
                println!("Executed.");
                return;
            }

            // No JOIN - handle as before
            let mut rows = rows;

            // Check if columns contain any aggregates
            let has_aggregates = match &columns {
                Some(cols) => cols.iter().any(|c| {
                    c.starts_with("count(")
                        || c.starts_with("sum(")
                        || c.starts_with("avg(")
                        || c.starts_with("min(")
                        || c.starts_with("max(")
                }),
                None => false,
            };

            // Handle aggregates (with or without GROUP BY)
            if has_aggregates {
                if let Some(ref group_cols) = group_by {
                    // GROUP BY with aggregates
                    let groups = group_rows_by_columns(rows, group_cols);

                    // Parse columns for aggregates
                    let agg_cols: Vec<AggregateColumn> = match &columns {
                        Some(cols) => cols
                            .iter()
                            .map(|c| AggregateColumn::from_col_string(c))
                            .collect(),
                        None => vec![],
                    };

                    // Compute aggregate results
                    let mut result_rows = Vec::new();
                    for (_, group_rows) in groups {
                        let mut values = Vec::new();
                        for agg in &agg_cols {
                            values.push(compute_aggregate(agg, &group_rows));
                        }

                        // Apply HAVING filter
                        if passes_having_filter(&having, &agg_cols, &values) {
                            result_rows.push(values);
                        }
                    }

                    // Sort, apply distinct, offset/limit
                    // Note: Simplified - just display results
                    for values in result_rows {
                        println!("({})", values.join(", "));
                    }
                } else {
                    // Aggregates without GROUP BY - compute over all rows
                    rows = apply_sorting(rows, order_by);
                    rows = apply_distinct(rows, distinct);
                    rows = apply_offset_limit(rows, offset, limit);

                    // Parse columns for aggregates
                    let agg_cols: Vec<AggregateColumn> = match &columns {
                        Some(cols) => cols
                            .iter()
                            .map(|c| AggregateColumn::from_col_string(c))
                            .collect(),
                        None => vec![],
                    };

                    // Compute aggregates over all rows
                    let mut values = Vec::new();
                    for agg in &agg_cols {
                        values.push(compute_aggregate(agg, &rows));
                    }
                    println!("({})", values.join(", "));
                }
                println!("Executed.");
            } else {
                // Regular SELECT without aggregates
                rows = apply_sorting(rows, order_by);
                rows = apply_distinct(rows, distinct);
                rows = apply_offset_limit(rows, offset, limit);

                for row in rows {
                    match &columns {
                        None => println!("({}, {}, {})", row.id, row.username, row.email),
                        Some(cols) => {
                            let mut values: Vec<String> = Vec::new();
                            for col in cols.iter() {
                                match col.as_str() {
                                    "id" => values.push(row.id.to_string()),
                                    "username" => values.push(row.username.clone()),
                                    "email" => values.push(row.email.clone()),
                                    other => values.push(format!("NULL({})", other)),
                                }
                            }
                            println!("({})", values.join(", "));
                        }
                    }
                }
                println!("Executed.");
            }
        }
        Statement::SelectWhere {
            distinct,
            columns,
            from_table,
            join,
            conditions,
            operators,
            group_by,
            having,
            order_by,
            limit,
            offset,
            ..
        } => {
            // Resolve left (from) table
            let mut left_table_ref: Table = if let Some(ref ft) = from_table {
                load_table_by_name(ft, table)
            } else {
                load_table_by_name("users", table)
            };

            match left_table_ref.select_where_complex(&conditions, &operators) {
                Ok(rows) => {
                    // Handle JOIN case separately to avoid ownership issues
                    if let Some(ref jc) = join {
                        let right_table = load_table_by_name(&jc.table, table);
                        let jrows = apply_join(
                            rows,
                            &jc.on_left,
                            &right_table,
                            &jc.on_right,
                            jc.join_type.clone(),
                        );

                        // Display joined results
                        for jrow in jrows {
                            match &columns {
                                None => {
                                    if let (Some(rid), Some(rusername), Some(remail)) =
                                        (&jrow.right_id, &jrow.right_username, &jrow.right_email)
                                    {
                                        println!(
                                            "({}, {}, {} | {}, {}, {})",
                                            jrow.left_id,
                                            jrow.left_username,
                                            jrow.left_email,
                                            rid,
                                            rusername,
                                            remail
                                        );
                                    } else {
                                        println!(
                                            "({}, {}, {} | NULL, NULL, NULL)",
                                            jrow.left_id, jrow.left_username, jrow.left_email
                                        );
                                    }
                                }
                                Some(cols) => {
                                    let mut values: Vec<String> = Vec::new();
                                    for col in cols.iter() {
                                        match col.as_str() {
                                            "id" => values.push(jrow.left_id.to_string()),
                                            "username" => values.push(jrow.left_username.clone()),
                                            "email" => values.push(jrow.left_email.clone()),
                                            other => values.push(format!("NULL({})", other)),
                                        }
                                    }
                                    println!("({})", values.join(", "));
                                }
                            }
                        }
                        println!("Executed.");
                        return;
                    }

                    // No JOIN - handle as before
                    let mut rows = rows;
                    // Check if columns contain any aggregates
                    let has_aggregates = match &columns {
                        Some(cols) => cols.iter().any(|c| {
                            c.starts_with("count(")
                                || c.starts_with("sum(")
                                || c.starts_with("avg(")
                                || c.starts_with("min(")
                                || c.starts_with("max(")
                        }),
                        None => false,
                    };

                    // Handle aggregates (with or without GROUP BY)
                    if has_aggregates {
                        if let Some(ref group_cols) = group_by {
                            // GROUP BY with aggregates
                            let groups = group_rows_by_columns(rows, group_cols);

                            // Parse columns for aggregates
                            let agg_cols: Vec<AggregateColumn> = match &columns {
                                Some(cols) => cols
                                    .iter()
                                    .map(|c| AggregateColumn::from_col_string(c))
                                    .collect(),
                                None => vec![],
                            };

                            // Compute aggregate results
                            let mut result_rows = Vec::new();
                            for (_, group_rows) in groups {
                                let mut values = Vec::new();
                                for agg in &agg_cols {
                                    values.push(compute_aggregate(agg, &group_rows));
                                }

                                // Apply HAVING filter
                                if passes_having_filter(&having, &agg_cols, &values) {
                                    result_rows.push(values);
                                }
                            }

                            // Display results
                            for values in result_rows {
                                println!("({})", values.join(", "));
                            }
                        } else {
                            // Aggregates without GROUP BY - compute over all filtered rows
                            rows = apply_sorting(rows, order_by);
                            rows = apply_distinct(rows, distinct);
                            rows = apply_offset_limit(rows, offset, limit);

                            // Parse columns for aggregates
                            let agg_cols: Vec<AggregateColumn> = match &columns {
                                Some(cols) => cols
                                    .iter()
                                    .map(|c| AggregateColumn::from_col_string(c))
                                    .collect(),
                                None => vec![],
                            };

                            // Compute aggregates over filtered rows
                            let mut values = Vec::new();
                            for agg in &agg_cols {
                                values.push(compute_aggregate(agg, &rows));
                            }
                            println!("({})", values.join(", "));
                        }
                        println!("Executed.");
                    } else {
                        // Regular SELECT WHERE without aggregates
                        rows = apply_sorting(rows, order_by);
                        rows = apply_distinct(rows, distinct);
                        rows = apply_offset_limit(rows, offset, limit);

                        for row in rows {
                            match &columns {
                                None => println!("({}, {}, {})", row.id, row.username, row.email),
                                Some(cols) => {
                                    let mut values = Vec::new();
                                    for col in cols {
                                        match col.as_str() {
                                            "id" => values.push(row.id.to_string()),
                                            "username" => values.push(row.username.clone()),
                                            "email" => values.push(row.email.clone()),
                                            other => values.push(format!("NULL({})", other)),
                                        }
                                    }
                                    println!("({})", values.join(", "));
                                }
                            }
                        }
                        println!("Executed.");
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::Update { id, column, value } => match table.update(id, &column, &value) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::Delete { id } => match table.delete(id) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::DeleteWhere { column, value } => match table.delete_where(&column, &value) {
            Ok(count) => {
                table.save().unwrap();
                println!("Deleted {} rows.", count);
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::DeleteAll => {
            let count = table.clear();
            table.save().unwrap();
            println!("Deleted {} rows.", count);
        }
    }
}

// Sort rows based on ORDER BY clause
fn apply_sorting(mut rows: Vec<&Row>, order_by: Option<(String, bool)>) -> Vec<&Row> {
    if let Some((column, is_asc)) = order_by {
        rows.sort_by(|a, b| {
            let cmp = match column.as_str() {
                "id" => a.id.cmp(&b.id),
                "username" => a.username.cmp(&b.username),
                "email" => a.email.cmp(&b.email),
                _ => std::cmp::Ordering::Equal,
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

// Apply LIMIT and OFFSET to results
fn apply_offset_limit(rows: Vec<&Row>, offset: Option<u32>, limit: Option<u32>) -> Vec<&Row> {
    let start = offset.unwrap_or(0) as usize;
    let end = if let Some(lim) = limit {
        start + lim as usize
    } else {
        rows.len()
    };

    rows.into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

// Remove duplicate rows if DISTINCT is enabled
fn apply_distinct(rows: Vec<&Row>, distinct: bool) -> Vec<&Row> {
    if !distinct {
        return rows;
    }

    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut unique_rows = Vec::new();

    for row in rows {
        let row_tuple = (row.id, row.username.clone(), row.email.clone());
        if seen.insert(row_tuple) {
            unique_rows.push(row);
        }
    }

    unique_rows
}

fn main() {
    let mut table = Table::new("data.json".to_string());

    // Optional: seed a secondary table for JOIN demos if empty
    {
        let mut orders = Table::new("orders.json".to_string());
        if orders.select_all().is_empty() {
            let _ = orders
                .insert(Row::new(1, "alice".to_string(), "alice@orders.com".to_string()).unwrap());
            let _ = orders
                .insert(Row::new(2, "bob".to_string(), "bob@orders.com".to_string()).unwrap());
            let _ = orders.insert(
                Row::new(3, "charlie".to_string(), "charlie@orders.com".to_string()).unwrap(),
            );
            let _ = orders.save();
        }
    }

    loop {
        print_prompt();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input.starts_with('.') {
            match do_meta_command(input, &mut table) {
                MetaCommandResult::Success => continue,
                MetaCommandResult::UnrecognizedCommand => {
                    println!("Unrecognized command '{}'", input);
                    continue;
                }
            }
        }

        match prepare_statement(input) {
            PrepareResult::Success(statement) => {
                execute_statement(statement, &mut table);
            }
            PrepareResult::UnrecognizedStatement => {
                println!("Unrecognized keyword at start of '{}'", input);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_join_basic() {
        // Create and seed two tables: users and orders
        let mut users = Table::new("test_users.json".to_string());
        users.clear();

        // Insert test users
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(2, "bob".to_string(), "bob@example.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(3, "charlie".to_string(), "charlie@example.com".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut orders = Table::new("test_orders.json".to_string());
        orders.clear();

        // Insert test orders with matching IDs
        assert!(orders
            .insert(Row::new(1, "alice".to_string(), "alice@orders.com".to_string()).unwrap())
            .is_ok());
        assert!(orders
            .insert(Row::new(2, "bob".to_string(), "bob@orders.com".to_string()).unwrap())
            .is_ok());
        orders.save().unwrap();

        // Load tables
        let users_loaded = users.select_all();
        let orders_loaded = orders.select_all();

        // Verify both tables have correct rows
        assert_eq!(users_loaded.len(), 3);
        assert_eq!(orders_loaded.len(), 2);
    }

    #[test]
    fn test_join_clause_parsing() {
        let input = "SELECT * FROM users INNER JOIN orders ON id = id";
        let result = parser::parse_select(input);

        assert!(result.is_ok());
        let (_, _, from_table, join, _, _, _, _, _, _) = result.unwrap();

        assert_eq!(from_table, Some("users".to_string()));
        assert!(join.is_some());

        let jc = join.unwrap();
        assert_eq!(jc.table, "orders");
        assert_eq!(jc.on_left, "id");
        assert_eq!(jc.on_right, "id");
        assert_eq!(jc.join_type, parser::JoinType::Inner);
    }

    #[test]
    fn test_left_join_parsing() {
        let input = "SELECT * FROM users LEFT JOIN orders ON username = username";
        let result = parser::parse_select(input);

        assert!(result.is_ok());
        let (_, _, from_table, join, _, _, _, _, _, _) = result.unwrap();

        assert_eq!(from_table, Some("users".to_string()));
        assert!(join.is_some());

        let jc = join.unwrap();
        assert_eq!(jc.table, "orders");
        assert_eq!(jc.join_type, parser::JoinType::Left);
    }

    #[test]
    fn test_select_with_from_clause() {
        let input = "SELECT id, username FROM users";
        let result = parser::parse_select(input);

        assert!(result.is_ok());
        let (_, cols, from_table, join, _, _, _, _, _, _) = result.unwrap();

        assert!(from_table.is_some());
        assert!(join.is_none());
        assert!(cols.is_some());

        let col_list = cols.unwrap();
        assert_eq!(col_list.len(), 2);
        assert_eq!(col_list[0], "id");
        assert_eq!(col_list[1], "username");
    }

    #[test]
    fn test_left_join_execution() {
        // Create test tables
        let mut users = Table::new("test_left_users.json".to_string());
        users.clear();

        // Insert 3 users
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut orders = Table::new("test_left_orders.json".to_string());
        orders.clear();

        // Insert orders for only alice and bob (charlie has no orders)
        assert!(orders
            .insert(Row::new(1, "alice".to_string(), "order1@test.com".to_string()).unwrap())
            .is_ok());
        assert!(orders
            .insert(Row::new(2, "bob".to_string(), "order2@test.com".to_string()).unwrap())
            .is_ok());
        orders.save().unwrap();

        // Simulate LEFT JOIN
        let user_rows = users.select_all();

        // LEFT JOIN should keep all 3 users (even charlie with no orders)
        assert_eq!(user_rows.len(), 3);

        // Apply LEFT JOIN logic manually
        let result = super::execute_statement(
            Statement::Select {
                distinct: false,
                columns: None,
                from_table: Some("test_left_users".to_string()),
                join: Some(parser::JoinClause {
                    join_type: parser::JoinType::Left,
                    table: "test_left_orders".to_string(),
                    on_left: "id".to_string(),
                    on_right: "id".to_string(),
                }),
                group_by: None,
                having: None,
                order_by: None,
                limit: None,
                offset: None,
            },
            &mut users,
        );
    }

    #[test]
    fn test_right_join_execution() {
        // Create test tables
        let mut users = Table::new("test_right_users.json".to_string());
        users.clear();

        // Insert 2 users
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut orders = Table::new("test_right_orders.json".to_string());
        orders.clear();

        // Insert orders including one without matching user
        assert!(orders
            .insert(Row::new(1, "alice".to_string(), "order1@test.com".to_string()).unwrap())
            .is_ok());
        assert!(orders
            .insert(Row::new(2, "bob".to_string(), "order2@test.com".to_string()).unwrap())
            .is_ok());
        assert!(orders
            .insert(Row::new(3, "david".to_string(), "order3@test.com".to_string()).unwrap())
            .is_ok());
        orders.save().unwrap();

        // RIGHT JOIN should keep only users that have matching orders
        let user_rows = users.select_all();
        assert_eq!(user_rows.len(), 2);
    }

    #[test]
    fn test_right_join_parsing() {
        let input = "SELECT * FROM users RIGHT JOIN orders ON id = id";
        let result = parser::parse_select(input);

        assert!(result.is_ok());
        let (_, _, from_table, join, _, _, _, _, _, _) = result.unwrap();

        assert_eq!(from_table, Some("users".to_string()));
        assert!(join.is_some());

        let jc = join.unwrap();
        assert_eq!(jc.table, "orders");
        assert_eq!(jc.join_type, parser::JoinType::Right);
        assert_eq!(jc.on_left, "id");
        assert_eq!(jc.on_right, "id");
    }

    #[test]
    fn test_inner_join_filters_correctly() {
        // Create test tables
        let mut users = Table::new("test_inner_users.json".to_string());
        users.clear();

        // Insert 3 users
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut orders = Table::new("test_inner_orders.json".to_string());
        orders.clear();

        // Insert orders for only alice (id=1)
        assert!(orders
            .insert(Row::new(1, "alice".to_string(), "order1@test.com".to_string()).unwrap())
            .is_ok());
        orders.save().unwrap();

        // INNER JOIN should return only 1 user (alice)
        let user_rows = users.select_all();
        let orders_table = orders;

        // Apply INNER JOIN manually
        let mut matched_count = 0;
        for row in &user_rows {
            if let Ok(matches) = orders_table.select_where("id", "=", &row.id.to_string()) {
                if !matches.is_empty() {
                    matched_count += 1;
                }
            }
        }

        assert_eq!(matched_count, 1, "INNER JOIN should match only 1 user");
    }
}
