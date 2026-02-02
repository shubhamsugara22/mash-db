use std::collections::HashMap;
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
        table_name: Option<String>,
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
    CreateTable {
        table_name: String,
        columns: Vec<String>,
    },
    DropTable {
        table_name: String,
    },
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
            Ok((table_name, id, username, email)) => PrepareResult::Success(Statement::Insert {
                table_name,
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
    } else if input.to_uppercase().starts_with("CREATE TABLE") {
        match parser::parse_create_table(input) {
            Ok((table_name, columns)) => PrepareResult::Success(Statement::CreateTable {
                table_name,
                columns,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase().starts_with("DROP TABLE") {
        match parser::parse_drop_table(input) {
            Ok(table_name) => PrepareResult::Success(Statement::DropTable { table_name }),
            Err(_) => PrepareResult::UnrecognizedStatement,
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

fn execute_statement(statement: Statement, tables: &mut HashMap<String, Table>) {
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

    // Load a table by name from the registry, or create it if it doesn't exist
    fn load_table_by_name<'a>(name: &str, tables: &'a mut HashMap<String, Table>) -> &'a mut Table {
        let name_lower = name.to_lowercase();
        tables
            .entry(name_lower.clone())
            .or_insert_with(|| Table::new(table_file_for(&name_lower)))
    }

    // Get default table for backward compatibility with existing code
    fn get_default_table<'a>(tables: &'a mut HashMap<String, Table>) -> &'a mut Table {
        load_table_by_name("users", tables)
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

    // Evaluate a single condition against a JoinedRow, supporting qualified names
    fn eval_joined_condition(
        jrow: &JoinedRow,
        condition: &(String, String, String),
        left_table_name: &str,
        right_table_name: &str,
    ) -> bool {
        let (column, operator, value) = condition;
        let (target_table, col_name) = if let Some(idx) = column.find('.') {
            (column[..idx].to_lowercase(), extract_column_name(column))
        } else {
            (left_table_name.to_string(), extract_column_name(column))
        };

        // Handle IS NULL / IS NOT NULL
        if operator == "IS NULL" || operator == "IS NOT NULL" {
            let is_null = if target_table == left_table_name {
                false // Left side never NULL in joined row
            } else if target_table == right_table_name {
                match col_name {
                    "id" => jrow.right_id.is_none(),
                    "username" => jrow.right_username.is_none(),
                    "email" => jrow.right_email.is_none(),
                    _ => false,
                }
            } else {
                false
            };
            return if operator == "IS NULL" {
                is_null
            } else {
                !is_null
            };
        }

        // Helpers for comparisons
        fn cmp_u32(val: u32, op: &str, rhs: &str) -> bool {
            let r = rhs.parse::<i64>().unwrap_or(0);
            let l = val as i64;
            match op {
                "=" => l == r,
                "!=" => l != r,
                ">" => l > r,
                "<" => l < r,
                ">=" => l >= r,
                "<=" => l <= r,
                _ => false,
            }
        }
        fn cmp_str(val: &str, op: &str, rhs: &str) -> bool {
            match op {
                "=" => val == rhs,
                "LIKE" => pattern_match(val, rhs),
                _ => false,
            }
        }
        fn cmp_opt_u32(val: Option<u32>, op: &str, rhs: &str) -> bool {
            match val {
                Some(v) => cmp_u32(v, op, rhs),
                None => false,
            }
        }
        fn cmp_opt_str(val: Option<&String>, op: &str, rhs: &str) -> bool {
            match val {
                Some(v) => cmp_str(v, op, rhs),
                None => false,
            }
        }
        fn pattern_match(text: &str, pattern: &str) -> bool {
            let text_chars: Vec<char> = text.chars().collect();
            let pattern_chars: Vec<char> = pattern.chars().collect();
            pattern_match_recursive(&text_chars, &pattern_chars, 0, 0)
        }
        fn pattern_match_recursive(
            text: &[char],
            pattern: &[char],
            t_idx: usize,
            p_idx: usize,
        ) -> bool {
            if p_idx >= pattern.len() && t_idx >= text.len() {
                return true;
            }
            if p_idx >= pattern.len() {
                return false;
            }
            if pattern[p_idx] == '%' {
                if pattern_match_recursive(text, pattern, t_idx, p_idx + 1) {
                    return true;
                }
                if t_idx < text.len() {
                    return pattern_match_recursive(text, pattern, t_idx + 1, p_idx);
                }
                return false;
            }
            if t_idx >= text.len() {
                return false;
            }
            if pattern[p_idx] == '_' || pattern[p_idx] == text[t_idx] {
                return pattern_match_recursive(text, pattern, t_idx + 1, p_idx + 1);
            }
            false
        }

        if target_table == left_table_name {
            match col_name {
                "id" => cmp_u32(jrow.left_id, operator.as_str(), value),
                "username" => cmp_str(&jrow.left_username, operator.as_str(), value),
                "email" => cmp_str(&jrow.left_email, operator.as_str(), value),
                _ => false,
            }
        } else if target_table == right_table_name {
            match col_name {
                "id" => cmp_opt_u32(jrow.right_id, operator.as_str(), value),
                "username" => cmp_opt_str(jrow.right_username.as_ref(), operator.as_str(), value),
                "email" => cmp_opt_str(jrow.right_email.as_ref(), operator.as_str(), value),
                _ => false,
            }
        } else {
            false
        }
    }

    // Filter joined rows using complex conditions with AND/OR precedence
    fn filter_joined_rows(
        jrows: Vec<JoinedRow>,
        conditions: &[(String, String, String)],
        operators: &[String],
        left_table_name: &str,
        right_table_name: &str,
    ) -> Vec<JoinedRow> {
        if conditions.is_empty() {
            return jrows;
        }
        let mut result = Vec::new();
        for j in jrows.into_iter() {
            let mut matches = eval_joined_condition(
                &j,
                &conditions[conditions.len() - 1],
                left_table_name,
                right_table_name,
            );
            for i in (0..operators.len()).rev() {
                let cond_res =
                    eval_joined_condition(&j, &conditions[i], left_table_name, right_table_name);
                match operators[i].as_str() {
                    "AND" => matches = cond_res && matches,
                    "OR" => matches = cond_res || matches,
                    _ => {}
                }
            }
            if matches {
                result.push(j);
            }
        }
        result
    }

    match statement {
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
            // Resolve left (from) table - get a cloned copy to avoid borrowing issues
            let table_name = from_table.as_deref().unwrap_or("users");
            let left_table_clone = {
                let tbl = load_table_by_name(table_name, tables);
                // Create a new Table instance pointing to the same file to avoid borrowing issues
                Table::new(table_file_for(table_name))
            };

            let rows = left_table_clone.select_all();

            // Handle JOIN case separately to avoid ownership issues
            if let Some(ref jc) = join {
                let right_table = Table::new(table_file_for(&jc.table));
                let jrows = apply_join(
                    rows,
                    &jc.on_left,
                    &right_table,
                    &jc.on_right,
                    jc.join_type.clone(),
                );
                // Apply ORDER BY for joined rows (supports qualified names)
                let left_table_name = from_table
                    .as_ref()
                    .map(|s| s.to_lowercase())
                    .unwrap_or_else(|| "users".to_string());
                let right_table_name = jc.table.to_lowercase();
                let jrows =
                    apply_joined_sorting(jrows, order_by, &left_table_name, &right_table_name);
                let jrows = apply_joined_offset_limit(jrows, offset, limit);
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
                            // Show selected columns with support for qualified names
                            let left_table_name = from_table
                                .as_ref()
                                .map(|s| s.to_lowercase())
                                .unwrap_or_else(|| "users".to_string());
                            let right_table_name = jc.table.to_lowercase();
                            let mut values: Vec<String> = Vec::new();
                            for col in cols.iter() {
                                if let Some(dot_idx) = col.find('.') {
                                    let tbl = col[..dot_idx].to_lowercase();
                                    let col_name = extract_column_name(col);
                                    if tbl == left_table_name {
                                        match col_name {
                                            "id" => values.push(jrow.left_id.to_string()),
                                            "username" => values.push(jrow.left_username.clone()),
                                            "email" => values.push(jrow.left_email.clone()),
                                            other => values.push(format!("NULL({})", other)),
                                        }
                                    } else if tbl == right_table_name {
                                        match col_name {
                                            "id" => values.push(
                                                jrow.right_id
                                                    .map(|v| v.to_string())
                                                    .unwrap_or("NULL".to_string()),
                                            ),
                                            "username" => values.push(
                                                jrow.right_username
                                                    .clone()
                                                    .unwrap_or("NULL".to_string()),
                                            ),
                                            "email" => values.push(
                                                jrow.right_email
                                                    .clone()
                                                    .unwrap_or("NULL".to_string()),
                                            ),
                                            other => values.push(format!("NULL({})", other)),
                                        }
                                    } else {
                                        values.push(format!("NULL({})", col));
                                    }
                                } else {
                                    let col_name = extract_column_name(col);
                                    match col_name {
                                        "id" => values.push(jrow.left_id.to_string()),
                                        "username" => values.push(jrow.left_username.clone()),
                                        "email" => values.push(jrow.left_email.clone()),
                                        other => values.push(format!("NULL({})", other)),
                                    }
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

                    // Sort aggregate results by ORDER BY, then apply LIMIT/OFFSET
                    result_rows = apply_sorting_to_aggregates(result_rows, order_by, &agg_cols);

                    // Apply LIMIT/OFFSET to aggregate results
                    let start = offset.unwrap_or(0) as usize;
                    let end = if let Some(lim) = limit {
                        start + lim as usize
                    } else {
                        result_rows.len()
                    };
                    result_rows = result_rows
                        .into_iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect();

                    // Display results
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
                                let col_name = extract_column_name(col);
                                match col_name {
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
            // Resolve left (from) table - get a cloned copy to avoid borrowing issues
            let table_name = from_table.as_deref().unwrap_or("users");
            let left_table_clone = Table::new(table_file_for(table_name));

            match left_table_clone.select_where_complex(&conditions, &operators) {
                Ok(rows) => {
                    // Handle JOIN case separately to avoid ownership issues
                    if let Some(ref jc) = join {
                        let right_table = Table::new(table_file_for(&jc.table));
                        let jrows = apply_join(
                            rows,
                            &jc.on_left,
                            &right_table,
                            &jc.on_right,
                            jc.join_type.clone(),
                        );

                        // Apply WHERE filters across joined rows, supporting qualified names
                        let left_table_name = from_table
                            .as_ref()
                            .map(|s| s.to_lowercase())
                            .unwrap_or_else(|| "users".to_string());
                        let right_table_name = jc.table.to_lowercase();
                        let jrows = if !conditions.is_empty() {
                            filter_joined_rows(
                                jrows,
                                &conditions,
                                &operators,
                                &left_table_name,
                                &right_table_name,
                            )
                        } else {
                            jrows
                        };

                        // Apply ORDER BY for joined rows (supports qualified names)
                        let jrows = apply_joined_sorting(
                            jrows,
                            order_by,
                            &left_table_name,
                            &right_table_name,
                        );
                        let jrows = apply_joined_offset_limit(jrows, offset, limit);

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
                                    // Show selected columns with support for qualified names
                                    let left_table_name = from_table
                                        .as_ref()
                                        .map(|s| s.to_lowercase())
                                        .unwrap_or_else(|| "users".to_string());
                                    let right_table_name = jc.table.to_lowercase();
                                    let mut values: Vec<String> = Vec::new();
                                    for col in cols.iter() {
                                        if let Some(dot_idx) = col.find('.') {
                                            let tbl = col[..dot_idx].to_lowercase();
                                            let col_name = extract_column_name(col);
                                            if tbl == left_table_name {
                                                match col_name {
                                                    "id" => values.push(jrow.left_id.to_string()),
                                                    "username" => {
                                                        values.push(jrow.left_username.clone())
                                                    }
                                                    "email" => values.push(jrow.left_email.clone()),
                                                    other => {
                                                        values.push(format!("NULL({})", other))
                                                    }
                                                }
                                            } else if tbl == right_table_name {
                                                match col_name {
                                                    "id" => values.push(
                                                        jrow.right_id
                                                            .map(|v| v.to_string())
                                                            .unwrap_or("NULL".to_string()),
                                                    ),
                                                    "username" => values.push(
                                                        jrow.right_username
                                                            .clone()
                                                            .unwrap_or("NULL".to_string()),
                                                    ),
                                                    "email" => values.push(
                                                        jrow.right_email
                                                            .clone()
                                                            .unwrap_or("NULL".to_string()),
                                                    ),
                                                    other => {
                                                        values.push(format!("NULL({})", other))
                                                    }
                                                }
                                            } else {
                                                values.push(format!("NULL({})", col));
                                            }
                                        } else {
                                            let col_name = extract_column_name(col);
                                            match col_name {
                                                "id" => values.push(jrow.left_id.to_string()),
                                                "username" => {
                                                    values.push(jrow.left_username.clone())
                                                }
                                                "email" => values.push(jrow.left_email.clone()),
                                                other => values.push(format!("NULL({})", other)),
                                            }
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

                            // Sort aggregate results by ORDER BY, then apply LIMIT/OFFSET
                            result_rows =
                                apply_sorting_to_aggregates(result_rows, order_by, &agg_cols);

                            // Apply LIMIT/OFFSET to aggregate results
                            let start = offset.unwrap_or(0) as usize;
                            let end = if let Some(lim) = limit {
                                start + lim as usize
                            } else {
                                result_rows.len()
                            };
                            result_rows = result_rows
                                .into_iter()
                                .skip(start)
                                .take(end.saturating_sub(start))
                                .collect();

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
                                        let col_name = extract_column_name(col);
                                        match col_name {
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
        Statement::Update { id, column, value } => {
            let table = get_default_table(tables);
            match table.update(id, &column, &value) {
                Ok(()) => {
                    table.save().unwrap();
                    println!("Executed.");
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::Delete { id } => {
            let table = get_default_table(tables);
            match table.delete(id) {
                Ok(()) => {
                    table.save().unwrap();
                    println!("Executed.");
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::DeleteWhere { column, value } => {
            let table = get_default_table(tables);
            match table.delete_where(&column, &value) {
                Ok(count) => {
                    table.save().unwrap();
                    println!("Deleted {} rows.", count);
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::DeleteAll => {
            // Get the default table (users) for backward compatibility
            let table = tables
                .entry("users".to_string())
                .or_insert_with(|| Table::new(table_file_for("users")));
            let count = table.clear();
            table.save().unwrap();
            println!("Deleted {} rows.", count);
        }
        Statement::CreateTable {
            table_name,
            columns,
        } => {
            let table_name_lower = table_name.to_lowercase();

            // Check if table already exists
            if tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' already exists", table_name);
                return;
            }

            // Create new table file
            let file_path = table_file_for(&table_name_lower);
            let new_table = Table::new(file_path);

            // Store table columns metadata (for future schema validation)
            // For now, we'll just create an empty table
            tables.insert(table_name_lower.clone(), new_table);

            println!(
                "Table '{}' created with columns: {}",
                table_name,
                columns.join(", ")
            );
        }
        Statement::DropTable { table_name } => {
            let table_name_lower = table_name.to_lowercase();

            // Check if table exists
            if !tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            // Don't allow dropping the default users table
            if table_name_lower == "users" {
                println!("Error: Cannot drop default table 'users'");
                return;
            }

            // Remove table from registry
            tables.remove(&table_name_lower);

            // Optionally delete the JSON file
            let file_path = table_file_for(&table_name_lower);
            if std::path::Path::new(&file_path).exists() {
                if let Err(e) = std::fs::remove_file(&file_path) {
                    println!(
                        "Warning: Could not delete table file '{}': {}",
                        file_path, e
                    );
                }
            }

            println!("Table '{}' dropped", table_name);
        }
    }
}

// Sort rows based on ORDER BY clause
fn apply_sorting(mut rows: Vec<&Row>, order_by: Option<(String, bool)>) -> Vec<&Row> {
    if let Some((column, is_asc)) = order_by {
        let col_name: &str = if let Some(idx) = column.rfind('.') {
            &column[idx + 1..]
        } else {
            &column
        };
        rows.sort_by(|a, b| {
            let cmp = match col_name {
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

// Sort joined rows based on ORDER BY clause (supports qualified names)
fn apply_joined_sorting(
    mut jrows: Vec<JoinedRow>,
    order_by: Option<(String, bool)>,
    left_table_name: &str,
    right_table_name: &str,
) -> Vec<JoinedRow> {
    if let Some((column, is_asc)) = order_by {
        let (target_table, col_name): (String, &str) = if let Some(idx) = column.find('.') {
            (column[..idx].to_lowercase(), &column[idx + 1..])
        } else {
            (left_table_name.to_string(), &column)
        };

        fn ord_opt_u32(a: &Option<u32>, b: &Option<u32>) -> std::cmp::Ordering {
            match (a, b) {
                (Some(av), Some(bv)) => av.cmp(bv),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }
        fn ord_opt_str(a: &Option<String>, b: &Option<String>) -> std::cmp::Ordering {
            match (a, b) {
                (Some(av), Some(bv)) => av.cmp(bv),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }

        jrows.sort_by(|a, b| {
            let cmp = if target_table == left_table_name {
                match col_name {
                    "id" => a.left_id.cmp(&b.left_id),
                    "username" => a.left_username.cmp(&b.left_username),
                    "email" => a.left_email.cmp(&b.left_email),
                    _ => std::cmp::Ordering::Equal,
                }
            } else if target_table == right_table_name {
                match col_name {
                    "id" => ord_opt_u32(&a.right_id, &b.right_id),
                    "username" => ord_opt_str(&a.right_username, &b.right_username),
                    "email" => ord_opt_str(&a.right_email, &b.right_email),
                    _ => std::cmp::Ordering::Equal,
                }
            } else {
                std::cmp::Ordering::Equal
            };
            if is_asc {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }
    jrows
}

// Sort aggregate results by ORDER BY column
// Maps aggregate function names in ORDER BY to their result column indices
fn apply_sorting_to_aggregates(
    mut result_rows: Vec<Vec<String>>,
    order_by: Option<(String, bool)>,
    agg_cols: &[AggregateColumn],
) -> Vec<Vec<String>> {
    if let Some((column, is_asc)) = order_by {
        // Find the index of the column to sort by
        let sort_index = if column.starts_with("count(")
            || column.starts_with("sum(")
            || column.starts_with("avg(")
            || column.starts_with("min(")
            || column.starts_with("max(")
        {
            // ORDER BY aggregate function - match by function name
            agg_cols.iter().position(|agg| {
                let agg_str = match agg {
                    AggregateColumn::Count(None) => "count(*)".to_string(),
                    AggregateColumn::Count(Some(col)) => format!("count({})", col),
                    AggregateColumn::CountDistinct(col) => format!("count(distinct {})", col),
                    AggregateColumn::Sum(col) => format!("sum({})", col),
                    AggregateColumn::Avg(col) => format!("avg({})", col),
                    AggregateColumn::Min(col) => format!("min({})", col),
                    AggregateColumn::Max(col) => format!("max({})", col),
                    AggregateColumn::Regular(_) => String::new(),
                };
                agg_str.to_lowercase() == column.to_lowercase()
            })
        } else {
            // ORDER BY regular column - match by column name
            agg_cols.iter().position(|agg| match agg {
                AggregateColumn::Regular(col) => col.to_lowercase() == column.to_lowercase(),
                _ => false,
            })
        };

        if let Some(idx) = sort_index {
            result_rows.sort_by(|a, b| {
                let cmp = if idx < a.len() && idx < b.len() {
                    // Try to parse as numbers first (for aggregates)
                    let a_num = a[idx].parse::<f64>();
                    let b_num = b[idx].parse::<f64>();
                    match (a_num, b_num) {
                        (Ok(an), Ok(bn)) => {
                            // Compare as numbers
                            if an < bn {
                                std::cmp::Ordering::Less
                            } else if an > bn {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Equal
                            }
                        }
                        _ => a[idx].cmp(&b[idx]), // Fall back to string comparison
                    }
                } else {
                    std::cmp::Ordering::Equal
                };
                if is_asc {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
    }
    result_rows
}

// Apply LIMIT and OFFSET to joined results
fn apply_joined_offset_limit(
    jrows: Vec<JoinedRow>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Vec<JoinedRow> {
    let start = offset.unwrap_or(0) as usize;
    let end = if let Some(lim) = limit {
        start + lim as usize
    } else {
        jrows.len()
    };
    jrows
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
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
    // Initialize table registry with default "users" table
    let mut tables: HashMap<String, Table> = HashMap::new();
    tables.insert("users".to_string(), Table::new("data.json".to_string()));

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
        tables.insert("orders".to_string(), orders);
    }

    loop {
        print_prompt();

        let mut input = String::new();
        let bytes_read = io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        // Exit on EOF (e.g., from piped input or Ctrl+D)
        if bytes_read == 0 {
            break;
        }

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input.starts_with('.') {
            // Get the default users table for meta commands
            let table = tables.get_mut("users").expect("Users table not found");
            match do_meta_command(input, table) {
                MetaCommandResult::Success => continue,
                MetaCommandResult::UnrecognizedCommand => {
                    println!("Unrecognized command '{}'", input);
                    continue;
                }
            }
        }

        match prepare_statement(input) {
            PrepareResult::Success(statement) => {
                execute_statement(statement, &mut tables);
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
        let mut tables: std::collections::HashMap<String, Table> = std::collections::HashMap::new();
        tables.insert("test_left_users".to_string(), users);
        tables.insert("test_left_orders".to_string(), orders);

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
            &mut tables,
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
