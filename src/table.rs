const COLUMN_USERNAME_SIZE: usize = 255;
const COLUMN_EMAIL_SIZE: usize = 255;

use crate::pager::Pager;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Operator {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

impl Operator {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "=" => Some(Operator::Eq),
            "!=" => Some(Operator::Ne),
            ">" => Some(Operator::Gt),
            "<" => Some(Operator::Lt),
            ">=" => Some(Operator::Ge),
            "<=" => Some(Operator::Le),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: u32,
    pub username: String,
    pub email: String,
    pub extras: HashMap<String, String>,
}

impl Row {
    pub fn new(id: u32, username: String, email: String) -> Result<Self, String> {
        if username.len() > COLUMN_USERNAME_SIZE {
            return Err(format!(
                "Username too long (max {} chars)",
                COLUMN_USERNAME_SIZE
            ));
        }
        if email.len() > COLUMN_EMAIL_SIZE {
            return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
        }
        Ok(Row {
            id,
            username,
            email,
            extras: HashMap::new(),
        })
    }

    pub fn from_values(schema: &[String], values: Vec<String>) -> Result<Self, String> {
        if schema.len() != values.len() {
            return Err("Column count does not match schema".to_string());
        }

        let mut id: u32 = 0;
        let mut username = String::new();
        let mut email = String::new();
        let mut extras: HashMap<String, String> = HashMap::new();

        for (col, val) in schema.iter().zip(values.into_iter()) {
            match col.as_str() {
                "id" => {
                    id = val
                        .parse::<u32>()
                        .map_err(|_| "Invalid id value".to_string())?;
                }
                "username" => {
                    if val.len() > COLUMN_USERNAME_SIZE {
                        return Err(format!(
                            "Username too long (max {} chars)",
                            COLUMN_USERNAME_SIZE
                        ));
                    }
                    username = val;
                }
                "email" => {
                    if val.len() > COLUMN_EMAIL_SIZE {
                        return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
                    }
                    email = val;
                }
                _ => {
                    extras.insert(col.clone(), val);
                }
            }
        }

        Ok(Row {
            id,
            username,
            email,
            extras,
        })
    }

    pub fn get_value(&self, column: &str) -> Option<String> {
        match column {
            "id" => Some(self.id.to_string()),
            "username" => Some(self.username.clone()),
            "email" => Some(self.email.clone()),
            _ => self.extras.get(column).cloned(),
        }
    }

    /// Evaluate a column expression, applying string functions if present.
    /// Supports `upper(col)`, `lower(col)`, `length(col)`, `__case__:...` (CASE WHEN).
    pub fn eval_col(&self, col_expr: &str) -> Option<String> {
        if let Some(inner) = col_expr
            .strip_prefix("upper(")
            .and_then(|s| s.strip_suffix(')'))
        {
            self.get_value(inner).map(|v| v.to_uppercase())
        } else if let Some(inner) = col_expr
            .strip_prefix("lower(")
            .and_then(|s| s.strip_suffix(')'))
        {
            self.get_value(inner).map(|v| v.to_lowercase())
        } else if let Some(inner) = col_expr
            .strip_prefix("length(")
            .and_then(|s| s.strip_suffix(')'))
        {
            self.get_value(inner).map(|v| v.len().to_string())
        } else if let Some(rest) = col_expr.strip_prefix("__case__:") {
            let parts: Vec<&str> = rest.split('\x1E').collect();
            let mut result: Option<String> = None;
            for part in &parts {
                if let Some(ev) = part.strip_prefix("__else__\x1F") {
                    if result.is_none() {
                        result = Some(ev.to_string());
                    }
                    break;
                }
                let fields: Vec<&str> = part.splitn(4, '\x1F').collect();
                if fields.len() == 4 && result.is_none() {
                    let (col, op, val, then_val) = (fields[0], fields[1], fields[2], fields[3]);
                    let row_val = self.get_value(col).unwrap_or_default();
                    let matched = match op {
                        "=" => row_val == val,
                        "!=" | "<>" => row_val != val,
                        ">" => row_val
                            .parse::<f64>()
                            .ok()
                            .zip(val.parse::<f64>().ok())
                            .map_or(row_val.as_str() > val, |(a, b)| a > b),
                        "<" => row_val
                            .parse::<f64>()
                            .ok()
                            .zip(val.parse::<f64>().ok())
                            .map_or(row_val.as_str() < val, |(a, b)| a < b),
                        ">=" => row_val
                            .parse::<f64>()
                            .ok()
                            .zip(val.parse::<f64>().ok())
                            .map_or(row_val.as_str() >= val, |(a, b)| a >= b),
                        "<=" => row_val
                            .parse::<f64>()
                            .ok()
                            .zip(val.parse::<f64>().ok())
                            .map_or(row_val.as_str() <= val, |(a, b)| a <= b),
                        _ => false,
                    };
                    if matched {
                        result = Some(then_val.to_string());
                    }
                }
            }
            result.or(Some("NULL".to_string()))
        } else if let Some(rest) = col_expr.strip_prefix("__coalesce__:") {
            let mut parts = rest.splitn(2, '\x1F');
            let col = parts.next().unwrap_or("");
            let default_val = parts.next().unwrap_or("").to_string();
            match self.get_value(col) {
                Some(v) if !v.is_empty() => Some(v),
                _ => Some(default_val),
            }
        } else if let Some(rest) = col_expr.strip_prefix("__nullif__:") {
            let mut parts = rest.splitn(2, '\x1F');
            let col = parts.next().unwrap_or("");
            let val = parts.next().unwrap_or("");
            let row_val = self.get_value(col).unwrap_or_default();
            if row_val == val {
                Some("NULL".to_string())
            } else {
                Some(row_val)
            }
        } else if let Some(col) = col_expr.strip_prefix("__trim__:") {
            Some(self.get_value(col).unwrap_or_default().trim().to_string())
        } else if let Some(rest) = col_expr.strip_prefix("__cast__:") {
            let mut parts = rest.splitn(2, '\x1F');
            let col = parts.next().unwrap_or("");
            let cast_type = parts.next().unwrap_or("TEXT");
            let raw = self.get_value(col).unwrap_or_default();
            match cast_type {
                "INTEGER" | "INT" => {
                    let n = raw.parse::<f64>().ok().map(|f| (f as i64).to_string()).unwrap_or(raw);
                    Some(n)
                }
                "REAL" | "FLOAT" | "DOUBLE" => {
                    let n = raw.parse::<f64>().ok().map(|f| f.to_string()).unwrap_or(raw);
                    Some(n)
                }
                _ => Some(raw), // TEXT and anything else: no conversion needed
            }
        } else if let Some(rest) = col_expr.strip_prefix("__concat__:") {
            let parts: Vec<&str> = rest.splitn(2, '\x1F').collect();
            let resolve = |arg: &str| -> String {
                if let Some(col) = arg.strip_prefix("c:") {
                    self.get_value(col).unwrap_or_default()
                } else if let Some(s) = arg.strip_prefix("s:") {
                    s.to_string()
                } else if let Some(n) = arg.strip_prefix("n:") {
                    n.to_string()
                } else {
                    arg.to_string()
                }
            };
            let a = parts.first().map(|s| resolve(s)).unwrap_or_default();
            let b = parts.get(1).map(|s| resolve(s)).unwrap_or_default();
            Some(format!("{}{}", a, b))
        } else if let Some(rest) = col_expr.strip_prefix("__if__:") {
            let parts: Vec<&str> = rest.splitn(5, '\x1F').collect();
            if parts.len() < 5 {
                return Some("NULL".to_string());
            }
            let (col, op, cmp, then_val, else_val) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
            let row_val = self.get_value(col).unwrap_or_default();
            let to_f = |s: &str| s.parse::<f64>().unwrap_or(0.0);
            let matches = match op {
                "="  => row_val == cmp,
                "!=" => row_val != cmp,
                ">"  => to_f(&row_val) > to_f(cmp),
                "<"  => to_f(&row_val) < to_f(cmp),
                ">=" => to_f(&row_val) >= to_f(cmp),
                "<=" => to_f(&row_val) <= to_f(cmp),
                _    => false,
            };
            if matches { Some(then_val.to_string()) } else { Some(else_val.to_string()) }
        } else if let Some(col) = col_expr.strip_prefix("__abs__:") {
            let raw = self.get_value(col).unwrap_or_default();
            let result = raw.parse::<f64>().ok()
                .map(|f| {
                    let abs = f.abs();
                    if abs.fract() == 0.0 && abs < 1e15 { (abs as i64).to_string() } else { abs.to_string() }
                })
                .unwrap_or(raw);
            Some(result)
        } else if let Some(rest) = col_expr.strip_prefix("__round__:") {
            let mut parts = rest.splitn(2, '\x1F');
            let col = parts.next().unwrap_or("");
            let digits: i32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let raw = self.get_value(col).unwrap_or_default();
            let result = raw.parse::<f64>().ok()
                .map(|f| {
                    let factor = 10f64.powi(digits);
                    let rounded = (f * factor).round() / factor;
                    if digits <= 0 {
                        (rounded as i64).to_string()
                    } else {
                        format!("{:.prec$}", rounded, prec = digits as usize)
                    }
                })
                .unwrap_or(raw);
            Some(result)
        } else if let Some(rest) = col_expr.strip_prefix("__substr__:") {
            let parts: Vec<&str> = rest.splitn(3, '\x1F').collect();
            let col = parts.first().copied().unwrap_or("");
            let start: usize = parts.get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
                .saturating_sub(1); // SQL is 1-based
            let len: usize = parts.get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            let raw = self.get_value(col).unwrap_or_default();
            let result: String = raw.chars().skip(start).take(len).collect();
            Some(result)
        } else {
            self.get_value(col_expr)
        }
    }

    #[allow(dead_code)]
    pub fn get_value_ref(&self, column: &str) -> Option<&str> {
        match column {
            "username" => Some(self.username.as_str()),
            "email" => Some(self.email.as_str()),
            _ => self.extras.get(column).map(|v| v.as_str()),
        }
    }
}

#[derive(Debug)]
pub struct Table {
    pager: Pager,
    schema: Vec<String>,
    has_id: bool,
    has_username: bool,
    has_email: bool,
    id_index: BTreeMap<u32, (usize, usize)>, // Maps id to (page_index, row_index)
    username_index: BTreeMap<String, Vec<(usize, usize)>>, // Maps username to list of (page, row) indices
    email_index: BTreeMap<String, Vec<(usize, usize)>>, // Maps email to list of (page, row) indices
}

impl Table {
    pub fn new(file_path: String, schema: Vec<String>) -> Self {
        let pager = Pager::new(file_path);
        let mut table = Table {
            pager,
            schema,
            has_id: false,
            has_username: false,
            has_email: false,
            id_index: BTreeMap::new(),
            username_index: BTreeMap::new(),
            email_index: BTreeMap::new(),
        };
        table.update_schema_flags();
        table.rebuild_indexes();
        table
    }

    pub fn schema(&self) -> &Vec<String> {
        &self.schema
    }

    #[allow(dead_code)]
    pub fn set_schema(&mut self, schema: Vec<String>) {
        self.schema = schema;
        self.update_schema_flags();
        self.rebuild_indexes();
    }

    fn update_schema_flags(&mut self) {
        self.has_id = self.schema.iter().any(|c| c == "id");
        self.has_username = self.schema.iter().any(|c| c == "username");
        self.has_email = self.schema.iter().any(|c| c == "email");
    }

    /// Insert a row into the table.
    /// Returns an error if a row with the same `id` already exists.
    pub fn insert(&mut self, row: Row) -> Result<(), String> {
        if self.has_id && self.id_index.contains_key(&row.id) {
            return Err(format!("Duplicate id {}", row.id));
        }
        // Find the page to add to
        let page_index =
            if self.pager.pages.is_empty() || self.pager.pages.last().unwrap().is_full() {
                self.pager.pages.len()
            } else {
                self.pager.pages.len() - 1
            };
        let row_index = if page_index < self.pager.pages.len() {
            self.pager.pages[page_index].rows.len()
        } else {
            0
        };
        self.pager.add_row(row.clone());
        let pos = (page_index, row_index);
        if self.has_id {
            self.id_index.insert(row.id, pos);
        }
        if self.has_username {
            self.username_index
                .entry(row.username.clone())
                .or_default()
                .push(pos);
        }
        if self.has_email {
            self.email_index
                .entry(row.email.clone())
                .or_default()
                .push(pos);
        }
        Ok(())
    }

    pub fn select_all(&self) -> Vec<&Row> {
        self.pager.pages.iter().flat_map(|p| &p.rows).collect()
    }

    pub fn select_where(
        &self,
        column: &str,
        operator: &str,
        value: &str,
    ) -> Result<Vec<&Row>, String> {
        let op = Operator::from_str(operator).ok_or("Invalid operator".to_string())?;
        let mut result = Vec::new();

        match column {
            "id" => {
                if !self.has_id {
                    return Err("Column 'id' does not exist".to_string());
                }
                let id_val = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid id value".to_string())?;
                match op {
                    Operator::Eq => {
                        if let Some(&(page_index, row_index)) = self.id_index.get(&id_val) {
                            result.push(&self.pager.pages[page_index].rows[row_index]);
                        }
                    }
                    Operator::Ne => {
                        for (id, &(page_index, row_index)) in &self.id_index {
                            if *id != id_val {
                                result.push(&self.pager.pages[page_index].rows[row_index]);
                            }
                        }
                    }
                    Operator::Gt => {
                        for (_id, &(page_index, row_index)) in self.id_index.range((id_val + 1)..) {
                            result.push(&self.pager.pages[page_index].rows[row_index]);
                        }
                    }
                    Operator::Lt => {
                        for (_id, &(page_index, row_index)) in self.id_index.range(..id_val) {
                            result.push(&self.pager.pages[page_index].rows[row_index]);
                        }
                    }
                    Operator::Ge => {
                        for (_id, &(page_index, row_index)) in self.id_index.range(id_val..) {
                            result.push(&self.pager.pages[page_index].rows[row_index]);
                        }
                    }
                    Operator::Le => {
                        for (_id, &(page_index, row_index)) in self.id_index.range(..=id_val) {
                            result.push(&self.pager.pages[page_index].rows[row_index]);
                        }
                    }
                }
            }
            "username" => {
                if !self.has_username {
                    return Err("Column 'username' does not exist".to_string());
                }
                if op != Operator::Eq {
                    return Err("Only = supported for username".to_string());
                }
                if let Some(positions) = self.username_index.get(value) {
                    for &(page_index, row_index) in positions {
                        result.push(&self.pager.pages[page_index].rows[row_index]);
                    }
                }
            }
            "email" => {
                if !self.has_email {
                    return Err("Column 'email' does not exist".to_string());
                }
                if op != Operator::Eq {
                    return Err("Only = supported for email".to_string());
                }
                if let Some(positions) = self.email_index.get(value) {
                    for &(page_index, row_index) in positions {
                        result.push(&self.pager.pages[page_index].rows[row_index]);
                    }
                }
            }
            _ => {
                if !self.schema.iter().any(|c| c == column) {
                    return Err(format!("Invalid column '{}'", column));
                }
                for row in self.select_all() {
                    let matches = Self::compare_values(row.get_value(column), operator, value);
                    if matches {
                        result.push(row);
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn select_where_complex(
        &self,
        conditions: &[(String, String, String)],
        operators: &[String],
    ) -> Result<Vec<&Row>, String> {
        let mut result = Vec::new();

        for row in self.select_all() {
            // Start with the last condition
            let mut matches = self.evaluate_condition(row, &conditions[conditions.len() - 1]);

            // Apply operators in reverse order to give AND higher precedence
            for i in (0..operators.len()).rev() {
                let cond_result = self.evaluate_condition(row, &conditions[i]);
                match operators[i].as_str() {
                    "AND" => matches = cond_result && matches,
                    "OR" => matches = cond_result || matches,
                    _ => return Err("Invalid logical operator".to_string()),
                }
            }

            if matches {
                result.push(row);
            }
        }

        Ok(result)
    }

    fn evaluate_condition(&self, row: &Row, condition: &(String, String, String)) -> bool {
        let (column, operator, value) = condition;

        // Handle IS NULL / IS NOT NULL (always false for non-joined rows since all fields exist)
        if operator == "IS NULL" {
            return false; // Single table rows never have NULL fields
        }
        if operator == "IS NOT NULL" {
            return true; // Single table rows always have non-NULL fields
        }

        // Handle BETWEEN operator - value format is "min,max"
        if operator == "BETWEEN" {
            let parts: Vec<&str> = value.split(',').collect();
            if parts.len() != 2 {
                return false;
            }
            let min_val: f64 = parts[0].parse().unwrap_or(0.0);
            let max_val: f64 = parts[1].parse().unwrap_or(0.0);
            let row_val = row.eval_col(column);
            if let Some(rv) = row_val {
                if let Ok(num) = rv.parse::<f64>() {
                    return num >= min_val && num <= max_val;
                }
            }
            false
        } else if operator == "IN" {
            let values: Vec<&str> = value.split(',').map(|v| v.trim()).collect();
            if let Some(rv) = row.eval_col(column) {
                return values.iter().any(|v| *v == rv);
            }
            false
        } else if operator == "NOT_IN" {
            let values: Vec<&str> = value.split(',').map(|v| v.trim()).collect();
            if let Some(rv) = row.eval_col(column) {
                return !values.iter().any(|v| *v == rv);
            }
            true // NULL not in any list
        } else if operator == "CONST_TRUE" {
            true
        } else if operator == "CONST_FALSE" {
            false
        } else {
            if let Some(rv) = row.eval_col(column) {
                if operator == "LIKE" {
                    return Self::pattern_match(&rv, value);
                }
                return Self::compare_values(Some(rv), operator, value);
            }
            false
        }
    }

    fn compare_values(row_value: Option<String>, operator: &str, expected: &str) -> bool {
        let rv = match row_value {
            Some(v) => v,
            None => return false,
        };

        if let (Ok(left), Ok(right)) = (rv.parse::<f64>(), expected.parse::<f64>()) {
            match operator {
                "=" => left == right,
                "!=" => left != right,
                ">" => left > right,
                "<" => left < right,
                ">=" => left >= right,
                "<=" => left <= right,
                _ => false,
            }
        } else {
            match operator {
                "=" => rv == expected,
                "!=" => rv != expected,
                _ => false,
            }
        }
    }

    /// Pattern matching for LIKE operator
    /// Supports % (zero or more characters) and _ (single character) wildcards
    fn pattern_match(text: &str, pattern: &str) -> bool {
        let text_chars: Vec<char> = text.chars().collect();
        let pattern_chars: Vec<char> = pattern.chars().collect();

        Self::pattern_match_recursive(&text_chars, &pattern_chars, 0, 0)
    }

    fn pattern_match_recursive(
        text: &[char],
        pattern: &[char],
        t_idx: usize,
        p_idx: usize,
    ) -> bool {
        // Both exhausted - match
        if p_idx >= pattern.len() && t_idx >= text.len() {
            return true;
        }

        // Pattern exhausted but text remains - no match
        if p_idx >= pattern.len() {
            return false;
        }

        // Check for % wildcard
        if pattern[p_idx] == '%' {
            // Try matching % with zero characters (skip % in pattern)
            if Self::pattern_match_recursive(text, pattern, t_idx, p_idx + 1) {
                return true;
            }
            // Try matching % with one or more characters
            if t_idx < text.len() {
                return Self::pattern_match_recursive(text, pattern, t_idx + 1, p_idx);
            }
            return false;
        }

        // Text exhausted but pattern has non-% characters - no match
        if t_idx >= text.len() {
            return false;
        }

        // Check for _ wildcard or exact character match
        if pattern[p_idx] == '_' || pattern[p_idx] == text[t_idx] {
            return Self::pattern_match_recursive(text, pattern, t_idx + 1, p_idx + 1);
        }

        false
    }

    /// Update a row by id.
    /// Returns an error if the id doesn't exist or the value is invalid for the column.
    pub fn update(&mut self, id: u32, column: &str, value: &str) -> Result<(), String> {
        if !self.has_id {
            return Err("Cannot update without id column".to_string());
        }
        if let Some(&(page_index, row_index)) = self.id_index.get(&id) {
            let row = &mut self.pager.pages[page_index].rows[row_index];
            match column {
                "username" => {
                    if !self.has_username {
                        return Err("Column 'username' does not exist".to_string());
                    }
                    if value.len() > COLUMN_USERNAME_SIZE {
                        return Err(format!(
                            "Username too long (max {} chars)",
                            COLUMN_USERNAME_SIZE
                        ));
                    }
                    // Remove from old username index
                    self.username_index
                        .get_mut(&row.username)
                        .unwrap()
                        .retain(|&p| p != (page_index, row_index));
                    row.username = value.to_string();
                    // Add to new username index
                    self.username_index
                        .entry(row.username.clone())
                        .or_default()
                        .push((page_index, row_index));
                }
                "email" => {
                    if !self.has_email {
                        return Err("Column 'email' does not exist".to_string());
                    }
                    if value.len() > COLUMN_EMAIL_SIZE {
                        return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
                    }
                    // Remove from old email index
                    self.email_index
                        .get_mut(&row.email)
                        .unwrap()
                        .retain(|&p| p != (page_index, row_index));
                    row.email = value.to_string();
                    // Add to new email index
                    self.email_index
                        .entry(row.email.clone())
                        .or_default()
                        .push((page_index, row_index));
                }
                "id" => return Err("Cannot update id".to_string()),
                _ => {
                    if !self.schema.iter().any(|c| c == column) {
                        return Err(format!("Unknown column '{}'", column));
                    }
                    row.extras.insert(column.to_string(), value.to_string());
                }
            }
            Ok(())
        } else {
            Err(format!("Row with id {} not found", id))
        }
    }
    pub fn delete(&mut self, id: u32) -> Result<(), String> {
        if !self.has_id {
            return Err("Cannot delete without id column".to_string());
        }
        if let Some((page_index, row_index)) = self.id_index.remove(&id) {
            self.pager.pages[page_index].rows.remove(row_index);
            // Rebuild indexes after removal
            self.rebuild_indexes();
            Ok(())
        } else {
            Err(format!("Row with id {} not found", id))
        }
    }
    pub fn delete_where(&mut self, column: &str, value: &str) -> Result<usize, String> {
        let mut deleted_count = 0;

        match column {
            "id" => {
                if !self.has_id {
                    return Err("Column 'id' does not exist".to_string());
                }
                let id = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid id value".to_string())?;
                return match self.delete(id) {
                    Ok(_) => Ok(1),
                    Err(_) => Ok(0),
                };
            }
            "username" => {
                if !self.has_username {
                    return Err("Column 'username' does not exist".to_string());
                }
                for page in &mut self.pager.pages {
                    page.rows.retain(|row| {
                        if row.username == value {
                            deleted_count += 1;
                            false
                        } else {
                            true
                        }
                    });
                }
                self.rebuild_indexes();
            }
            "email" => {
                if !self.has_email {
                    return Err("Column 'email' does not exist".to_string());
                }
                for page in &mut self.pager.pages {
                    page.rows.retain(|row| {
                        if row.email == value {
                            deleted_count += 1;
                            false
                        } else {
                            true
                        }
                    });
                }
                self.rebuild_indexes();
            }
            _ => {
                if !self.schema.iter().any(|c| c == column) {
                    return Err(format!("Invalid column '{}'", column));
                }
                for page in &mut self.pager.pages {
                    page.rows.retain(|row| {
                        if row.extras.get(column).map(|v| v == value).unwrap_or(false) {
                            deleted_count += 1;
                            false
                        } else {
                            true
                        }
                    });
                }
                self.rebuild_indexes();
            }
        }
        Ok(deleted_count)
    }
    pub fn rebuild_indexes(&mut self) {
        self.id_index.clear();
        self.username_index.clear();
        self.email_index.clear();
        for (page_index, page) in self.pager.pages.iter().enumerate() {
            for (row_index, row) in page.rows.iter().enumerate() {
                let pos = (page_index, row_index);
                if self.has_id {
                    self.id_index.insert(row.id, pos);
                }
                if self.has_username {
                    self.username_index
                        .entry(row.username.clone())
                        .or_default()
                        .push(pos);
                }
                if self.has_email {
                    self.email_index
                        .entry(row.email.clone())
                        .or_default()
                        .push(pos);
                }
            }
        }
    }

    pub fn clear(&mut self) -> usize {
        let count = self.pager.pages.iter().map(|p| p.rows.len()).sum();
        self.pager.pages.clear();
        self.id_index.clear();
        self.username_index.clear();
        self.email_index.clear();
        count
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.pager.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_schema() -> Vec<String> {
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
        ]
    }

    #[test]
    fn row_validation_rejects_long_username_and_email() {
        let long_username = "a".repeat(COLUMN_USERNAME_SIZE + 1);
        let long_email = "b".repeat(COLUMN_EMAIL_SIZE + 1);

        assert!(Row::new(1, long_username.clone(), "ok@example.com".to_string()).is_err());
        assert!(Row::new(1, "ok".to_string(), long_email.clone()).is_err());
    }

    #[test]
    fn insert_prevents_duplicate_id() {
        let mut table = Table::new("test1.json".to_string(), default_schema());

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        assert!(table.insert(r1).is_ok());

        let r2 = Row::new(1, "bob".to_string(), "bob@example.com".to_string()).unwrap();
        let res = table.insert(r2);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Duplicate id 1");
    }

    #[test]
    fn update_modifies_existing_row() {
        let mut table = Table::new("test2.json".to_string(), default_schema());

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        table.insert(r1).unwrap();

        // Update username
        assert!(table.update(1, "username", "alice2").is_ok());
        let rows = table.select_all();
        assert_eq!(rows[0].username, "alice2");

        // Update email
        assert!(table.update(1, "email", "alice2@example.com").is_ok());
        let rows = table.select_all();
        assert_eq!(rows[0].email, "alice2@example.com");

        // Update non-existent id
        assert!(table.update(2, "username", "bob").is_err());

        // Update invalid column
        assert!(table.update(1, "invalid", "value").is_err());

        // Update id (should fail)
        assert!(table.update(1, "id", "2").is_err());
    }
    #[test]
    fn delete_removes_existing_row() {
        let mut table = Table::new("test3.json".to_string(), default_schema());

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        let r2 = Row::new(2, "bob".to_string(), "bob@example.com".to_string()).unwrap();

        table.insert(r1).unwrap();
        table.insert(r2).unwrap();

        // Delete existing row
        assert!(table.delete(1).is_ok());

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn delete_where_by_id_removes_row() {
        let mut table = Table::new("test4.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("id", "1").unwrap();
        assert_eq!(deleted, 1);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn delete_where_by_username() {
        let mut table = Table::new("test5.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("username", "alice").unwrap();
        assert_eq!(deleted, 2);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "bob");
    }
    #[test]
    fn delete_where_by_email() {
        let mut table = Table::new("test6.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("email", "b@b.com").unwrap();
        assert_eq!(deleted, 1);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].email, "a@a.com");
    }
    #[test]
    fn delete_where_invalid_column_fails() {
        let mut table = Table::new("test7.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let res = table.delete_where("invalid", "alice");
        assert!(res.is_err());
    }
    #[test]
    fn delete_where_no_matching_rows_returns_zero() {
        let mut table = Table::new("test8.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("username", "bob").unwrap();
        assert_eq!(deleted, 0);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
    }
    #[test]
    fn delete_all_removes_everything() {
        let mut table = Table::new("test9.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.clear();
        assert_eq!(deleted, 2);
        assert_eq!(table.select_all().len(), 0);
    }
    #[test]
    fn delete_all_on_empty_table() {
        let mut table = Table::new("test10.json".to_string(), default_schema());

        let deleted = table.clear();
        assert_eq!(deleted, 0);
        assert_eq!(table.select_all().len(), 0);
    }
    #[test]
    fn select_where_by_id() {
        let mut table = Table::new("test11.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("id", "=", "1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "alice");
    }
    #[test]
    fn select_where_by_username() {
        let mut table = Table::new("test12.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("username", "=", "alice").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].email, "a@a.com");
        assert_eq!(rows[1].email, "a2@a.com");
    }
    #[test]
    fn select_where_by_email() {
        let mut table = Table::new("test13.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("email", "=", "b@b.com").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn select_where_no_matches_returns_empty() {
        let mut table = Table::new("test14.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("username", "=", "bob").unwrap();
        assert_eq!(rows.len(), 0);
    }
    #[test]
    fn select_where_invalid_column_fails() {
        let mut table = Table::new("test15.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let res = table.select_where("invalid", "=", "alice");
        assert!(res.is_err());
    }
    #[test]
    fn select_where_complex_and() {
        let mut table = Table::new("test_and.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();

        let conditions = vec![
            ("id".to_string(), ">".to_string(), "1".to_string()),
            ("username".to_string(), "=".to_string(), "alice".to_string()),
        ];
        let operators = vec!["AND".to_string()];

        let rows = table.select_where_complex(&conditions, &operators).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 3);
        assert_eq!(rows[0].username, "alice");
    }

    #[test]
    fn select_where_complex_or() {
        let mut table = Table::new("test_or.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "charlie".to_string(), "c@c.com".to_string()).unwrap())
            .unwrap();

        let conditions = vec![
            ("id".to_string(), "=".to_string(), "1".to_string()),
            (
                "username".to_string(),
                "=".to_string(),
                "charlie".to_string(),
            ),
        ];
        let operators = vec!["OR".to_string()];

        let rows = table.select_where_complex(&conditions, &operators).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.id == 1));
        assert!(rows.iter().any(|r| r.username == "charlie"));
    }

    #[test]
    fn select_where_complex_mixed() {
        let mut table = Table::new("test_mixed.json".to_string(), default_schema());

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(4, "charlie".to_string(), "c@c.com".to_string()).unwrap())
            .unwrap();

        let conditions = vec![
            ("id".to_string(), ">".to_string(), "1".to_string()),
            ("username".to_string(), "=".to_string(), "alice".to_string()),
            ("id".to_string(), "!=".to_string(), "4".to_string()),
        ];
        let operators = vec!["AND".to_string(), "OR".to_string()];

        let rows = table.select_where_complex(&conditions, &operators).unwrap();
        // Should match: (id > 1 AND username = alice) OR id != 4
        // id=3 matches the AND part, id=2 matches the OR part (since id != 4)
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.id == 2));
        assert!(rows.iter().any(|r| r.id == 3));
    }
}
