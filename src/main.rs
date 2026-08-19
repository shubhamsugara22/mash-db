#![allow(unused_variables)]
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

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

// P² streaming quantile estimator for a single quantile `p` (one-pass, constant memory)
struct P2Estimator {
    p: f64,
    count: usize,
    buffer: Vec<f64>,    // store first 5 values
    q: [f64; 5],         // heights
    n: [usize; 5],       // positions
    n_desired: [f64; 5], // desired positions
}

impl P2Estimator {
    fn new(p: f64) -> Self {
        P2Estimator {
            p,
            count: 0,
            buffer: Vec::with_capacity(5),
            q: [0.0; 5],
            n: [0; 5],
            n_desired: [0.0; 5],
        }
    }

    fn initialize_from_buffer(&mut self) {
        self.buffer
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for i in 0..5 {
            self.q[i] = self.buffer[i];
            self.n[i] = i + 1;
        }
        self.update_desired_positions();
    }

    fn update_desired_positions(&mut self) {
        if self.count < 1 {
            return;
        }
        let pi = [0.0, self.p / 2.0, self.p, (1.0 + self.p) / 2.0, 1.0];
        for i in 0..5 {
            self.n_desired[i] = 1.0 + (self.count as f64 - 1.0) * pi[i];
        }
    }

    fn add(&mut self, x: f64) {
        self.count += 1;
        if self.count <= 5 {
            self.buffer.push(x);
            if self.count == 5 {
                self.initialize_from_buffer();
            }
            return;
        }

        // count >=6 now
        // find k: index such that q[k] <= x < q[k+1]
        let k = if x < self.q[0] {
            0
        } else if x >= self.q[4] {
            3 // we'll increment markers 4..4 later
        } else {
            let mut kk = 0;
            for i in 0..4 {
                if x >= self.q[i] && x < self.q[i + 1] {
                    kk = i;
                    break;
                }
            }
            kk
        };

        // Update marker positions
        if x < self.q[0] {
            self.q[0] = x;
            self.n[0] = 1;
        } else if x > self.q[4] {
            self.q[4] = x;
            self.n[4] = self.count;
        }
        // increment n for markers > k
        for i in (k + 1)..5 {
            self.n[i] += 1;
        }
        // desired positions
        self.update_desired_positions();

        // adjust heights for i = 1..3
        for i in 1..4 {
            let d = self.n_desired[i] - (self.n[i] as f64);
            if (d >= 1.0 && (self.n[i + 1] - self.n[i]) > 1)
                || (d <= -1.0 && (self.n[i] - self.n[i - 1]) > 1)
            {
                let sign = if d > 0.0 { 1 } else { -1 };
                let ni = self.n[i] as f64;
                let nim1 = self.n[i - 1] as f64;
                let nip1 = self.n[i + 1] as f64;
                let qi = self.q[i];
                let qim1 = self.q[i - 1];
                let qip1 = self.q[i + 1];
                let d_f = sign as f64;

                let denom = nip1 - nim1;
                let delta = (d_f / denom)
                    * ((ni - nim1 + d_f) * (qip1 - qi) / (nip1 - ni)
                        + (nip1 - ni - d_f) * (qi - qim1) / (ni - nim1));

                let q_new = qi + delta;
                if q_new > qim1 && q_new < qip1 {
                    self.q[i] = q_new;
                } else {
                    // linear
                    if sign > 0 {
                        self.q[i] = qi + (qip1 - qi) / (nip1 - ni);
                    } else {
                        self.q[i] = qi + (qim1 - qi) / (nim1 - ni);
                    }
                }
                self.n[i] = ((self.n[i] as isize) + sign) as usize;
            }
        }
    }

    fn result(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else if self.count < 5 {
            // exact on buffer
            let mut b = self.buffer.clone();
            b.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let m = b.len();
            if m == 0 {
                None
            } else {
                let p = self.p;
                let rank = p * (m as f64 - 1.0);
                let lower = rank.floor() as usize;
                let upper = rank.ceil() as usize;
                if lower == upper {
                    Some(b[lower])
                } else {
                    let frac = rank - (lower as f64);
                    Some(b[lower] + frac * (b[upper] - b[lower]))
                }
            }
        } else {
            Some(self.q[2])
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
    StringAgg(String, String),
    Median(String),
    Mode(String),
    Variance(String),
    StddevPop(String),
    StddevSamp(String),
    VarSamp(String),
    PercentileCont(String, String),
    PercentileDisc(String, String),
    ApproxPercentile(String, String),
    Corr(String, String),
}

impl AggregateColumn {
    fn from_col_string(col: &str) -> AggregateColumn {
        if col.starts_with("count(") && col.ends_with(")") {
            let inner = &col[6..col.len() - 1];
            if inner == "*" {
                AggregateColumn::Count(None)
            } else if let Some(col_name) = inner.strip_prefix("distinct ") {
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
        } else if col.starts_with("string_agg(") && col.ends_with(")") {
            let inner = &col[11..col.len() - 1];
            // expect expr,sep
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 2 {
                AggregateColumn::StringAgg(parts[0].to_string(), parts[1].to_string())
            } else {
                AggregateColumn::Regular(col.to_string())
            }
        } else if col.starts_with("median(") && col.ends_with(")") {
            let inner = &col[7..col.len() - 1];
            AggregateColumn::Median(inner.to_string())
        } else if col.starts_with("percentile_cont(") && col.ends_with(")") {
            let inner = &col[16..col.len() - 1];
            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                AggregateColumn::PercentileCont(parts[0].to_string(), parts[1].to_string())
            } else {
                AggregateColumn::Regular(col.to_string())
            }
        } else if col.starts_with("percentile_disc(") && col.ends_with(")") {
            let inner = &col[16..col.len() - 1];
            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                AggregateColumn::PercentileDisc(parts[0].to_string(), parts[1].to_string())
            } else {
                AggregateColumn::Regular(col.to_string())
            }
        } else if col.starts_with("approx_percentile(") && col.ends_with(")") {
            let inner = &col[17..col.len() - 1];
            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                AggregateColumn::ApproxPercentile(parts[0].to_string(), parts[1].to_string())
            } else {
                AggregateColumn::Regular(col.to_string())
            }
        } else if col.starts_with("mode(") && col.ends_with(")") {
            let inner = &col[5..col.len() - 1];
            AggregateColumn::Mode(inner.to_string())
        } else if col.starts_with("variance(") && col.ends_with(")") {
            let inner = &col[9..col.len() - 1];
            AggregateColumn::Variance(inner.to_string())
        } else if col.starts_with("stddev_pop(") && col.ends_with(")") {
            let inner = &col[11..col.len() - 1];
            AggregateColumn::StddevPop(inner.to_string())
        } else if col.starts_with("stddev_samp(") && col.ends_with(")") {
            let inner = &col[12..col.len() - 1];
            AggregateColumn::StddevSamp(inner.to_string())
        } else if col.starts_with("stddev(") && col.ends_with(")") {
            // STDDEV alias maps to sample stddev
            let inner = &col[7..col.len() - 1];
            AggregateColumn::StddevSamp(inner.to_string())
        } else if col.starts_with("var_samp(") && col.ends_with(")") {
            let inner = &col[9..col.len() - 1];
            AggregateColumn::VarSamp(inner.to_string())
        } else if col.starts_with("corr(") && col.ends_with(")") {
            let inner = &col[5..col.len() - 1];
            // expect two args separated by comma
            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                AggregateColumn::Corr(parts[0].to_string(), parts[1].to_string())
            } else {
                AggregateColumn::Regular(col.to_string())
            }
        } else {
            AggregateColumn::Regular(col.to_string())
        }
    }
}

#[allow(dead_code)]
enum MetaCommandResult {
    Success,
    UnrecognizedCommand,
}

#[allow(clippy::large_enum_variant)]
enum PrepareResult {
    Success(Statement),
    UnrecognizedStatement,
}

struct TransactionState {
    active: bool,
    table_snapshots: HashMap<String, Vec<Row>>,
    schema_snapshot: HashMap<String, Vec<String>>,
}

#[allow(clippy::type_complexity)]
enum Statement {
    BeginTransaction,
    CommitTransaction,
    RollbackTransaction,
    Insert {
        table_name: Option<String>,
        values: Vec<String>,
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
        from_table: Option<String>,
        join: Option<parser::JoinClause>,
        conditions: Vec<(String, String, String)>,
        operators: Vec<String>,
        group_by: Option<Vec<String>>,
        having: Option<(Vec<(String, String, String)>, Vec<String>)>,
        order_by: Option<(String, bool)>,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    SelectWithCTE {
        cte_name: String,
        cte_query: String,
        distinct: bool,
        columns: Option<Vec<String>>,
        from_table: Option<String>,
        join: Option<parser::JoinClause>,
        group_by: Option<Vec<String>>,
        having: Option<(Vec<(String, String, String)>, Vec<String>)>,
        order_by: Option<(String, bool)>,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    SelectWithCTEWhere {
        cte_name: String,
        cte_query: String,
        distinct: bool,
        columns: Option<Vec<String>>,
        from_table: Option<String>,
        join: Option<parser::JoinClause>,
        conditions: Vec<(String, String, String)>,
        operators: Vec<String>,
        group_by: Option<Vec<String>>,
        having: Option<(Vec<(String, String, String)>, Vec<String>)>,
        order_by: Option<(String, bool)>,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    Update {
        table_name: Option<String>,
        id: u32,
        assignments: Vec<(String, String)>,
    },
    Union {
        sql1: String,
        sql2: String,
        all: bool,
    },
    InsertSelect {
        table_name: String,
        select_sql: String,
    },
    Delete {
        table_name: Option<String>,
        id: u32,
    },
    DeleteWhere {
        table_name: Option<String>,
        column: String,
        value: String,
    },
    DeleteAll,
    CreateTable {
        table_name: String,
        columns: Vec<String>,
        primary_key: Option<String>,
        unique_columns: Vec<String>,
    },
    AlterTableRename {
        table_name: String,
        new_name: String,
    },
    AlterTableAddColumn {
        table_name: String,
        column: String,
    },
    AlterTableDropColumn {
        table_name: String,
        column: String,
    },
    DropTable {
        table_name: String,
    },
    TruncateTable {
        table_name: String,
    },
    CreateView {
        view_name: String,
        select_query: String,
    },
    DropView {
        view_name: String,
    },
    CreateIndex {
        index_name: String,
        table_name: String,
        column_name: String,
    },
    DropIndex {
        index_name: String,
    },
    Analyze {
        table_name: String,
    },
    ShowTables,
    ShowIndexes,
}

fn print_prompt() {
    print!("db > ");
    if let Err(e) = io::stdout().flush() {
        eprintln!("Error flushing prompt: {}", e);
    }
}

// Compute ROW_NUMBER values for any encoded __row_number__ columns in the select list.
pub fn compute_row_number_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__row_number__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        // parse encoded: __row_number__:partition_part\x1Forder_part
        let rest = wc.strip_prefix("__row_number__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        let partition_part = parts.get(0).copied().unwrap_or("");
        let order_part = parts.get(1).copied().unwrap_or("");

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // assign row numbers starting at 1
            let mut rn: u32 = 1;
            for (row, _orig_idx) in group_rows {
                mapping.insert(row.id, rn.to_string());
                rn += 1;
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute RANK values for any encoded __rank__ columns in the select list.
pub fn compute_rank_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for rank
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__rank__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        let rest = wc.strip_prefix("__rank__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        let partition_part = parts.get(0).copied().unwrap_or("");
        let order_part = parts.get(1).copied().unwrap_or("");

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // assign ranks: ties receive same rank, next rank increases by count (RANK semantics)
            let mut prev_vals: Option<Vec<String>> = None;
            let mut prev_rank: u32 = 0;
            let mut idx_counter: u32 = 0;
            for (row, _orig_idx) in &group_rows {
                idx_counter += 1;
                let mut key_vals: Vec<String> = Vec::new();
                for (col, _) in &order_specs {
                    key_vals.push(row.get_value(col).unwrap_or_default());
                }
                if prev_vals.is_none() || prev_vals.as_ref().unwrap() != &key_vals {
                    // new value, rank equals current position (which accounts for previous ties)
                    prev_rank = idx_counter;
                    prev_vals = Some(key_vals);
                }
                mapping.insert(row.id, prev_rank.to_string());
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute DENSE_RANK values for any encoded __dense_rank__ columns in the select list.
pub fn compute_dense_rank_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for dense rank
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__dense_rank__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        let rest = wc.strip_prefix("__dense_rank__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        let partition_part = parts.get(0).copied().unwrap_or("");
        let order_part = parts.get(1).copied().unwrap_or("");

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // assign dense ranks: ties receive same rank, next rank increments by 1 (no gaps)
            let mut prev_vals: Option<Vec<String>> = None;
            let mut dense_rank: u32 = 0;
            for (row, _orig_idx) in &group_rows {
                let mut key_vals: Vec<String> = Vec::new();
                for (col, _) in &order_specs {
                    key_vals.push(row.get_value(col).unwrap_or_default());
                }
                if prev_vals.is_none() || prev_vals.as_ref().unwrap() != &key_vals {
                    dense_rank += 1;
                    prev_vals = Some(key_vals);
                }
                mapping.insert(row.id, dense_rank.to_string());
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute FIRST_VALUE values for any encoded __first_value__ columns in the select list.
// FIRST_VALUE(column) returns the value of `column` from the first row in the window.
pub fn compute_first_value_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for FIRST_VALUE
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__first_value__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        // parse encoded: __first_value__:column\x1Fpartition_part\x1Ford_part
        let rest = wc.strip_prefix("__first_value__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        let first_column = parts.get(0).copied().unwrap_or("");
        let partition_part = parts.get(1).copied().unwrap_or("");
        let order_part = parts.get(2).copied().unwrap_or("");

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // Determine the first value for the group (if any)
            let first_val = if group_rows.is_empty() {
                "NULL".to_string()
            } else {
                group_rows[0]
                    .0
                    .get_value(&first_column.to_string())
                    .unwrap_or_else(|| "NULL".to_string())
            };

            for (row, _orig_idx) in &group_rows {
                mapping.insert(row.id, first_val.clone());
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute LAST_VALUE values for any encoded __last_value__ columns in the select list.
// LAST_VALUE(column) returns the value of `column` from the last row in the window.
pub fn compute_last_value_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for LAST_VALUE
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__last_value__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        // parse encoded: __last_value__:column\x1Fpartition_part\x1Ford_part
        let rest = wc.strip_prefix("__last_value__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        let last_column = parts.get(0).copied().unwrap_or("");
        let partition_part = parts.get(1).copied().unwrap_or("");
        let order_part = parts.get(2).copied().unwrap_or("");

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // Determine the last value for the group (if any)
            let last_val = if group_rows.is_empty() {
                "NULL".to_string()
            } else {
                let last_idx = group_rows.len() - 1;
                group_rows[last_idx]
                    .0
                    .get_value(&last_column.to_string())
                    .unwrap_or_else(|| "NULL".to_string())
            };

            for (row, _orig_idx) in &group_rows {
                mapping.insert(row.id, last_val.clone());
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute LEAD values for any encoded __lead__ columns in the select list.
// LEAD(column, offset, default) returns the value of column from offset rows ahead in the window.
pub fn compute_lead_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for LEAD
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__lead__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        // parse encoded: __lead__:column\x1Foffset\x1Fdefault\x1Fpartition_part\x1Forder_part
        let rest = wc.strip_prefix("__lead__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        if parts.len() < 5 {
            continue; // Invalid format
        }

        let lead_column = parts[0].to_string();
        let offset: usize = parts[1].parse().unwrap_or(1);
        let default_value = parts[2].to_string();
        let partition_part = parts[3];
        let order_part = parts[4];

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // Compute LEAD values: for each row, get the value from offset rows ahead
            for (current_idx, (row, _orig_idx)) in group_rows.iter().enumerate() {
                let lead_idx = current_idx + offset;
                let lead_value = if lead_idx < group_rows.len() {
                    // Get the lead row's column value
                    group_rows[lead_idx]
                        .0
                        .get_value(&lead_column)
                        .unwrap_or_else(|| default_value.clone())
                } else {
                    // Out of bounds, use default
                    default_value.clone()
                };
                mapping.insert(row.id, lead_value);
            }
        }

        result.insert(wc, mapping);
    }

    result
}

// Compute LAG values for any encoded __lag__ columns in the select list.
// LAG(column, offset, default) returns the value of column from offset rows behind in the window.
pub fn compute_lag_map(
    rows: &Vec<&Row>,
    columns: &Option<Vec<String>>,
) -> std::collections::HashMap<String, std::collections::HashMap<u32, String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashMap<u32, String>> =
        std::collections::HashMap::new();

    let cols: Vec<String> = match columns {
        Some(c) => c.clone(),
        None => Vec::new(),
    };

    // Find any window column expressions for LAG
    let window_cols: Vec<String> = cols
        .into_iter()
        .filter(|c| c.starts_with("__lag__:"))
        .collect();
    if window_cols.is_empty() {
        return result;
    }

    for wc in window_cols {
        // parse encoded: __lag__:column\x1Foffset\x1Fdefault\x1Fpartition_part\x1Forder_part
        let rest = wc.strip_prefix("__lag__:").unwrap_or("");
        let parts: Vec<&str> = rest.split('\x1F').collect();
        if parts.len() < 5 {
            continue; // Invalid format
        }

        let lag_column = parts[0].to_string();
        let offset: usize = parts[1].parse().unwrap_or(1);
        let default_value = parts[2].to_string();
        let partition_part = parts[3];
        let order_part = parts[4];

        let partition_cols: Vec<String> = if partition_part.is_empty() {
            Vec::new()
        } else {
            partition_part.split(',').map(|s| s.to_string()).collect()
        };
        let order_specs: Vec<(String, bool)> = if order_part.is_empty() {
            Vec::new()
        } else {
            order_part
                .split(',')
                .map(|s| {
                    if s.ends_with(":DESC") {
                        (s[..s.len() - 5].to_string(), false)
                    } else {
                        (s.to_string(), true)
                    }
                })
                .collect()
        };

        // Group rows by partition key
        let mut groups: std::collections::HashMap<String, Vec<(&Row, usize)>> =
            std::collections::HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            let mut key_parts: Vec<String> = Vec::new();
            for col in &partition_cols {
                key_parts.push(row.get_value(col).unwrap_or_else(|| "NULL".to_string()));
            }
            let key = key_parts.join("|");
            groups.entry(key).or_default().push((*row, idx));
        }

        let mut mapping: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        for (_k, mut group_rows) in groups.into_iter() {
            // sort group_rows according to order_specs
            group_rows.sort_by(|a, b| {
                let (ra, ia) = a;
                let (rb, ib) = b;
                for (col, asc) in &order_specs {
                    let va = ra.get_value(col).unwrap_or_default();
                    let vb = rb.get_value(col).unwrap_or_default();
                    // try numeric compare
                    if let (Ok(na), Ok(nb)) = (va.parse::<f64>(), vb.parse::<f64>()) {
                        if na < nb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if na > nb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    } else {
                        if va < vb {
                            return if *asc {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if va > vb {
                            return if *asc {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            };
                        }
                    }
                }
                // tie-breaker: original position
                ia.cmp(ib)
            });

            // Compute LAG values: for each row, get the value from offset rows behind
            for (current_idx, (row, _orig_idx)) in group_rows.iter().enumerate() {
                let lag_value = if current_idx >= offset {
                    // Get the lag row's column value
                    group_rows[current_idx - offset]
                        .0
                        .get_value(&lag_column)
                        .unwrap_or_else(|| default_value.clone())
                } else {
                    // Out of bounds, use default
                    default_value.clone()
                };
                mapping.insert(row.id, lag_value);
            }
        }

        result.insert(wc, mapping);
    }

    result
}

fn do_meta_command(input: &str, _table: &mut Table) -> MetaCommandResult {
    if input == ".exit" {
        println!("Bye!");
        std::process::exit(0);
    } else {
        MetaCommandResult::UnrecognizedCommand
    }
}

fn split_on_union(input: &str) -> Option<(String, String, bool)> {
    let upper = input.to_uppercase();
    let bytes = input.as_bytes();
    let ubytes = upper.as_bytes();
    let len = bytes.len();
    let mut depth: usize = 0;
    let mut i = 0;
    while i < len {
        if bytes[i] == b'(' {
            depth += 1;
        } else if bytes[i] == b')' {
            if depth > 0 {
                depth -= 1;
            }
        } else if depth == 0 {
            if i + 11 <= len && &ubytes[i..i + 11] == b" UNION ALL " {
                let s1 = input[..i].trim().to_string();
                let s2 = input[i + 11..].trim().to_string();
                return Some((s1, s2, true));
            }
            if i + 7 <= len && &ubytes[i..i + 7] == b" UNION " {
                let s1 = input[..i].trim().to_string();
                let s2 = input[i + 7..].trim().to_string();
                return Some((s1, s2, false));
            }
        }
        i += 1;
    }
    None
}

fn prepare_statement(input: &str) -> PrepareResult {
    let upper = input.to_uppercase();

    if upper == "BEGIN" || upper == "BEGIN TRANSACTION" {
        PrepareResult::Success(Statement::BeginTransaction)
    } else if upper == "COMMIT" || upper == "COMMIT TRANSACTION" {
        PrepareResult::Success(Statement::CommitTransaction)
    } else if upper == "ROLLBACK" || upper == "ROLLBACK TRANSACTION" {
        PrepareResult::Success(Statement::RollbackTransaction)
    } else if upper.starts_with("INSERT") {
        if upper.contains(" SELECT ") {
            match parser::parse_insert_select(input) {
                Ok((table_name, select_sql)) => {
                    return PrepareResult::Success(Statement::InsertSelect {
                        table_name,
                        select_sql,
                    });
                }
                Err(_) => return PrepareResult::UnrecognizedStatement,
            }
        }
        match parser::parse_insert(input) {
            Ok((table_name, values)) => {
                PrepareResult::Success(Statement::Insert { table_name, values })
            }
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("UPDATE") {
        match parser::parse_update(input) {
            Ok((table_name, id, assignments)) => PrepareResult::Success(Statement::Update {
                table_name,
                id,
                assignments,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("SELECT") {
        // Check if this is a CTE (WITH clause)
        if let Ok((Some(cte), main_query)) = parser::parse_cte(input) {
            // CTE found - return it as a special statement that will be handled during execution
            // For now, we'll process it as a regular SELECT but store the CTE info
            // We'll handle CTE substitution in the execute phase
            match parser::parse_select(&main_query) {
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
                )) => {
                    // Store CTE in a thread-local or pass it through execution
                    // For now, we'll modify from_table to include CTE execution
                    PrepareResult::Success(Statement::SelectWithCTE {
                        cte_name: cte.name,
                        cte_query: cte.query,
                        distinct,
                        columns: cols,
                        from_table,
                        join,
                        group_by,
                        having,
                        order_by,
                        limit,
                        offset,
                    })
                }
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
                )) => PrepareResult::Success(Statement::SelectWithCTEWhere {
                    cte_name: cte.name,
                    cte_query: cte.query,
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
        } else if let Some((sql1, sql2, all)) = split_on_union(input) {
            return PrepareResult::Success(Statement::Union { sql1, sql2, all });
        } else {
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
        }
    } else if upper == "DELETE ALL" {
        PrepareResult::Success(Statement::DeleteAll)
    } else if upper.starts_with("DELETE") {
        if upper.contains("WHERE") {
            match parser::parse_delete_where(input) {
                Ok((table_name, column, value)) => PrepareResult::Success(Statement::DeleteWhere {
                    table_name,
                    column,
                    value,
                }),
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        } else {
            match parser::parse_delete(input) {
                Ok((table_name, id)) => {
                    PrepareResult::Success(Statement::Delete { table_name, id })
                }
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        }
    } else if upper.starts_with("CREATE TABLE") {
        match parser::parse_create_table(input) {
            Ok((table_name, columns, primary_key, unique_columns)) => {
                PrepareResult::Success(Statement::CreateTable {
                    table_name,
                    columns,
                    primary_key,
                    unique_columns,
                })
            }
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("ALTER TABLE") {
        match parser::parse_alter_table(input) {
            Ok((table_name, action)) => match action {
                parser::AlterTableAction::Rename(new_name) => {
                    PrepareResult::Success(Statement::AlterTableRename {
                        table_name,
                        new_name,
                    })
                }
                parser::AlterTableAction::AddColumn(column) => {
                    PrepareResult::Success(Statement::AlterTableAddColumn { table_name, column })
                }
                parser::AlterTableAction::DropColumn(column) => {
                    PrepareResult::Success(Statement::AlterTableDropColumn { table_name, column })
                }
            },
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("DROP TABLE") {
        match parser::parse_drop_table(input) {
            Ok(table_name) => PrepareResult::Success(Statement::DropTable { table_name }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("TRUNCATE TABLE") {
        match parser::parse_truncate_table(input) {
            Ok(table_name) => PrepareResult::Success(Statement::TruncateTable { table_name }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("CREATE VIEW") {
        // CREATE VIEW view_name AS SELECT ...
        if let Some(as_idx) = input.to_uppercase().find(" AS ") {
            let view_part = &input[..as_idx].trim();
            let select_part = &input[as_idx + 4..].trim();

            // Extract view name: CREATE VIEW view_name
            let parts: Vec<&str> = view_part.split_whitespace().collect();
            if parts.len() >= 3 {
                let view_name = parts[2].to_string();
                let select_query = select_part.to_string();
                PrepareResult::Success(Statement::CreateView {
                    view_name,
                    select_query,
                })
            } else {
                PrepareResult::UnrecognizedStatement
            }
        } else {
            PrepareResult::UnrecognizedStatement
        }
    } else if upper.starts_with("DROP VIEW") {
        // DROP VIEW view_name
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() >= 3 {
            let view_name = parts[2].to_string();
            PrepareResult::Success(Statement::DropView { view_name })
        } else {
            PrepareResult::UnrecognizedStatement
        }
    } else if upper.starts_with("SHOW TABLES") {
        PrepareResult::Success(Statement::ShowTables)
    } else if upper.starts_with("SHOW INDEXES") || upper.starts_with("SHOW INDEX") {
        PrepareResult::Success(Statement::ShowIndexes)
    } else if upper.starts_with("CREATE INDEX") {
        match parser::parse_create_index(input) {
            Ok(def) => PrepareResult::Success(Statement::CreateIndex {
                index_name: def.index_name,
                table_name: def.table_name,
                column_name: def.column_name,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("DROP INDEX") {
        match parser::parse_drop_index(input) {
            Ok(index_name) => PrepareResult::Success(Statement::DropIndex { index_name }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if upper.starts_with("ANALYZE") {
        // Accept both "ANALYZE table_name" and "ANALYZE TABLE table_name"
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() >= 2 {
            let table_name = if parts.len() >= 3 && parts[1].to_uppercase() == "TABLE" {
                parts[2].to_string()
            } else {
                parts[1].to_string()
            };
            PrepareResult::Success(Statement::Analyze { table_name })
        } else {
            PrepareResult::UnrecognizedStatement
        }
    } else {
        PrepareResult::UnrecognizedStatement
    }
}

// Helper function to group rows by specific columns
fn group_rows_by_columns<'a>(
    rows: Vec<&'a Row>,
    group_by_cols: &[String],
    schema: &[String],
) -> std::collections::HashMap<String, Vec<&'a Row>> {
    let mut groups: std::collections::HashMap<String, Vec<&'a Row>> =
        std::collections::HashMap::new();

    for row in rows {
        let mut group_key = Vec::new();
        for col in group_by_cols {
            if schema.iter().any(|c| c == col) {
                group_key.push(row.get_value(col).unwrap_or("NULL".to_string()));
            } else {
                group_key.push("NULL".to_string());
            }
        }
        let key = group_key.join("|");
        groups.entry(key).or_default().push(row);
    }

    groups
}

// Helper function to compute aggregate value for a group of rows
fn compute_aggregate(agg: &AggregateColumn, rows: &[&Row], schema: &[String]) -> String {
    match agg {
        AggregateColumn::Regular(col) => {
            // For regular columns in GROUP BY, just return the first row's value
            if let Some(first_row) = rows.first() {
                if schema.iter().any(|c| c == col) {
                    first_row.get_value(col).unwrap_or("NULL".to_string())
                } else {
                    "NULL".to_string()
                }
            } else {
                "NULL".to_string()
            }
        }
        AggregateColumn::Count(col_opt) => match col_opt {
            None => rows.len().to_string(), // COUNT(*)
            Some(col) => {
                // COUNT(col) - count non-null values
                let count = rows
                    .iter()
                    .filter(|row| {
                        if schema.iter().any(|c| c == col) {
                            row.get_value(col).map(|v| !v.is_empty()).unwrap_or(false)
                        } else {
                            false
                        }
                    })
                    .count();
                count.to_string()
            }
        },
        AggregateColumn::CountDistinct(col) => {
            // COUNT(DISTINCT col) - count unique values
            let mut unique_values = std::collections::HashSet::new();
            if schema.iter().any(|c| c == col) {
                for row in rows {
                    if let Some(val) = row.get_value(col) {
                        if !val.is_empty() {
                            unique_values.insert(val);
                        }
                    }
                }
            }
            unique_values.len().to_string()
        }
        AggregateColumn::Sum(col) => {
            let sum: f64 = rows
                .iter()
                .filter_map(|row| row.get_value(col))
                .filter_map(|v| v.parse::<f64>().ok())
                .sum();
            format!("{:.0}", sum)
        }
        AggregateColumn::Avg(col) => {
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|row| row.get_value(col))
                .filter_map(|v| v.parse::<f64>().ok())
                .collect();
            if values.is_empty() {
                "NULL".to_string()
            } else {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                format!("{:.2}", avg)
            }
        }
        AggregateColumn::Min(col) => {
            if !schema.iter().any(|c| c == col) {
                return "NULL".to_string();
            }
            let values: Vec<String> = rows.iter().filter_map(|row| row.get_value(col)).collect();
            if values.is_empty() {
                return "NULL".to_string();
            }
            if values.iter().all(|v| v.parse::<f64>().is_ok()) {
                let nums: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.parse::<f64>().ok())
                    .collect();
                nums.iter()
                    .cloned()
                    .reduce(f64::min)
                    .map(|v| format!("{:.0}", v))
                    .unwrap_or_else(|| "NULL".to_string())
            } else {
                values.iter().min().cloned().unwrap_or("NULL".to_string())
            }
        }
        AggregateColumn::Max(col) => {
            if !schema.iter().any(|c| c == col) {
                return "NULL".to_string();
            }
            let values: Vec<String> = rows.iter().filter_map(|row| row.get_value(col)).collect();
            if values.is_empty() {
                return "NULL".to_string();
            }
            if values.iter().all(|v| v.parse::<f64>().is_ok()) {
                let nums: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.parse::<f64>().ok())
                    .collect();
                nums.iter()
                    .cloned()
                    .reduce(f64::max)
                    .map(|v| format!("{:.0}", v))
                    .unwrap_or_else(|| "NULL".to_string())
            } else {
                values.iter().max().cloned().unwrap_or("NULL".to_string())
            }
        }
        AggregateColumn::StringAgg(expr, sep) => {
            // Concatenate non-null values from rows using the separator
            if !schema.iter().any(|c| c == expr) {
                return "NULL".to_string();
            }
            let mut parts: Vec<String> = Vec::new();
            for row in rows {
                if let Some(val) = row.get_value(expr) {
                    if !val.is_empty() && val != "NULL" {
                        parts.push(val);
                    }
                }
            }
            if parts.is_empty() {
                "NULL".to_string()
            } else {
                parts.join(sep)
            }
        }
        AggregateColumn::Median(expr) => {
            // Calculate the median of numeric values
            if !schema.iter().any(|c| c == expr) {
                return "NULL".to_string();
            }
            let mut values: Vec<f64> = Vec::new();
            for row in rows {
                if let Some(val) = row.get_value(expr) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }
            if values.is_empty() {
                return "NULL".to_string();
            }
            // Sort values
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let len = values.len();
            if len % 2 == 1 {
                // Odd count: return middle value
                format!("{}", values[len / 2])
            } else {
                // Even count: return average of two middle values
                let mid1 = values[len / 2 - 1];
                let mid2 = values[len / 2];
                format!("{}", (mid1 + mid2) / 2.0)
            }
        }
        AggregateColumn::Mode(col_name) => {
            use std::collections::HashMap;
            let mut frequency: HashMap<String, usize> = HashMap::new();

            // Count frequencies of all non-NULL values
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    // Skip NULL or empty values
                    if !val.is_empty() && val != "NULL" {
                        *frequency.entry(val).or_insert(0) += 1;
                    }
                }
            }

            if frequency.is_empty() {
                "NULL".to_string()
            } else {
                // Find the value with the maximum frequency
                frequency
                    .into_iter()
                    .max_by_key(|&(_, count)| count)
                    .map(|(value, _)| value)
                    .unwrap_or_else(|| "NULL".to_string())
            }
        }
        AggregateColumn::Variance(col_name) => {
            let mut values: Vec<f64> = Vec::new();

            // Collect all non-NULL numeric values
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    // Skip NULL or empty values
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }

            if values.is_empty() {
                "NULL".to_string()
            } else {
                // Calculate variance: VAR(X) = Σ(xi - μ)² / n
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_squared_diffs: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
                let variance = sum_squared_diffs / values.len() as f64;
                variance.to_string()
            }
        }
        AggregateColumn::StddevPop(col_name) => {
            let mut values: Vec<f64> = Vec::new();

            // Collect all non-NULL numeric values
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    // Skip NULL or empty values
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }

            if values.is_empty() {
                "NULL".to_string()
            } else {
                // Population variance then square root
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_squared_diffs: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
                let variance = sum_squared_diffs / values.len() as f64;
                let stddev = variance.sqrt();
                stddev.to_string()
            }
        }
        AggregateColumn::StddevSamp(col_name) => {
            let mut values: Vec<f64> = Vec::new();

            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }

            if values.len() < 2 {
                "NULL".to_string()
            } else {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_squared_diffs: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
                let variance_samp = sum_squared_diffs / (values.len() as f64 - 1.0);
                let stddev = variance_samp.sqrt();
                stddev.to_string()
            }
        }
        AggregateColumn::VarSamp(col_name) => {
            let mut values: Vec<f64> = Vec::new();

            // Collect all non-NULL numeric values
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }

            if values.len() < 2 {
                "NULL".to_string()
            } else {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_squared_diffs: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
                let variance_samp = sum_squared_diffs / (values.len() as f64 - 1.0);
                variance_samp.to_string()
            }
        }

        AggregateColumn::PercentileDisc(col_name, perc_str) => {
            if !schema.iter().any(|c| c == col_name) {
                return "NULL".to_string();
            }
            let mut values: Vec<f64> = Vec::new();
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }
            if values.is_empty() {
                return "NULL".to_string();
            }
            let p: f64 = match perc_str.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return "NULL".to_string(),
            };
            if p < 0.0 || p > 1.0 {
                return "NULL".to_string();
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = values.len();
            // ordinal selection: ceil(p * n) -> position 1..n
            let mut pos = (p * n as f64).ceil() as usize;
            if pos == 0 {
                pos = 1;
            }
            if pos > n {
                pos = n;
            }
            values[pos - 1].to_string()
        }
        AggregateColumn::PercentileCont(col_name, perc_str) => {
            if !schema.iter().any(|c| c == col_name) {
                return "NULL".to_string();
            }
            let mut values: Vec<f64> = Vec::new();
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            values.push(num);
                        }
                    }
                }
            }
            if values.is_empty() {
                return "NULL".to_string();
            }
            let p: f64 = match perc_str.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return "NULL".to_string(),
            };
            if p < 0.0 || p > 1.0 {
                return "NULL".to_string();
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let m = values.len() as f64;
            if m == 1.0 {
                return values[0].to_string();
            }
            let rank = p * (m - 1.0);
            let lower = rank.floor() as usize;
            let upper = rank.ceil() as usize;
            if lower == upper {
                return values[lower].to_string();
            }
            let frac = rank - (lower as f64);
            let v = values[lower] + frac * (values[upper] - values[lower]);
            v.to_string()
        }

        AggregateColumn::ApproxPercentile(col_name, perc_str) => {
            // P² streaming estimator implementation
            if !schema.iter().any(|c| c == col_name) {
                return "NULL".to_string();
            }
            let p: f64 = match perc_str.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return "NULL".to_string(),
            };
            if p < 0.0 || p > 1.0 {
                return "NULL".to_string();
            }

            let mut estimator = P2Estimator::new(p);
            let mut any = false;
            for row in rows {
                if let Some(val) = row.get_value(col_name) {
                    if !val.is_empty() && val != "NULL" {
                        if let Ok(num) = val.parse::<f64>() {
                            estimator.add(num);
                            any = true;
                        }
                    }
                }
            }
            if !any {
                return "NULL".to_string();
            }
            if let Some(res) = estimator.result() {
                return res.to_string();
            }
            "NULL".to_string()
        }
        AggregateColumn::Corr(a_name, b_name) => {
            // Require both columns to exist in schema
            if !schema.iter().any(|c| c == a_name) || !schema.iter().any(|c| c == b_name) {
                return "NULL".to_string();
            }
            let mut pairs: Vec<(f64, f64)> = Vec::new();
            for row in rows {
                if let (Some(a_val), Some(b_val)) = (row.get_value(a_name), row.get_value(b_name)) {
                    if !a_val.is_empty() && a_val != "NULL" && !b_val.is_empty() && b_val != "NULL"
                    {
                        if let (Ok(x), Ok(y)) = (a_val.parse::<f64>(), b_val.parse::<f64>()) {
                            pairs.push((x, y));
                        }
                    }
                }
            }

            if pairs.len() < 2 {
                return "NULL".to_string();
            }

            let n = pairs.len() as f64;
            let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / n;
            let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / n;

            // Sample covariance and sample variances (denominator n-1)
            let mut cov_sum = 0.0;
            let mut var_x_sum = 0.0;
            let mut var_y_sum = 0.0;
            for (x, y) in &pairs {
                cov_sum += (x - mean_x) * (y - mean_y);
                var_x_sum += (x - mean_x).powi(2);
                var_y_sum += (y - mean_y).powi(2);
            }
            let denom = n - 1.0;
            let cov_samp = cov_sum / denom;
            let var_x_samp = var_x_sum / denom;
            let var_y_samp = var_y_sum / denom;

            if var_x_samp == 0.0 || var_y_samp == 0.0 {
                return "NULL".to_string();
            }

            let corr = cov_samp / (var_x_samp.sqrt() * var_y_samp.sqrt());
            corr.to_string()
        }
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
        AggregateColumn::StringAgg(a, b) => col_lower == format!("string_agg({},{})", a, b),
        AggregateColumn::Median(c) => col_lower == format!("median({})", c),
        AggregateColumn::Mode(c) => col_lower == format!("mode({})", c),
        AggregateColumn::Variance(c) => col_lower == format!("variance({})", c),
        AggregateColumn::StddevPop(c) => col_lower == format!("stddev_pop({})", c),
        AggregateColumn::StddevSamp(c) => {
            col_lower == format!("stddev_samp({})", c) || col_lower == format!("stddev({})", c)
        }
        AggregateColumn::PercentileCont(c, p) => {
            col_lower == format!("percentile_cont({},{})", c, p)
        }
        AggregateColumn::PercentileDisc(c, p) => {
            col_lower == format!("percentile_disc({},{})", c, p)
        }
        AggregateColumn::ApproxPercentile(c, p) => {
            col_lower == format!("approx_percentile({},{})", c, p)
        }
        AggregateColumn::VarSamp(c) => col_lower == format!("var_samp({})", c),
        AggregateColumn::Corr(a, b) => col_lower == format!("corr({},{})", a, b),
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
#[allow(clippy::type_complexity)]
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

fn execute_statement(
    statement: Statement,
    tables: &mut HashMap<String, Table>,
    schemas: &mut HashMap<String, Vec<String>>,
    views: &mut HashMap<String, String>,
    constraints: &mut HashMap<String, (Option<String>, Vec<String>)>,
    indexes: &mut HashMap<String, (String, String)>, // index_name -> (table_name, column_name)
    tx: &mut TransactionState,
) {
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

    fn get_schema_for(name: &str, schemas: &HashMap<String, Vec<String>>) -> Vec<String> {
        let name_lower = name.to_lowercase();
        schemas.get(&name_lower).cloned().unwrap_or_else(|| {
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ]
        })
    }

    // Load a table by name from the registry, or create it if it doesn't exist
    fn load_table_by_name<'a>(
        name: &str,
        tables: &'a mut HashMap<String, Table>,
        schemas: &HashMap<String, Vec<String>>,
    ) -> &'a mut Table {
        let name_lower = name.to_lowercase();
        let schema = get_schema_for(&name_lower, schemas);
        tables
            .entry(name_lower.clone())
            .or_insert_with(|| Table::new(table_file_for(&name_lower), schema))
    }

    // Get default table for backward compatibility with existing code
    fn get_default_table<'a>(
        tables: &'a mut HashMap<String, Table>,
        schemas: &HashMap<String, Vec<String>>,
    ) -> &'a mut Table {
        load_table_by_name("users", tables, schemas)
    }

    // Check if an index exists for (table, column) and return the index name if found
    fn find_index_for<'a>(
        table_name: &str,
        column_name: &str,
        indexes: &'a HashMap<String, (String, String)>,
    ) -> Option<&'a str> {
        let tbl = table_name.to_lowercase();
        let col = column_name.to_lowercase();
        for (idx_name, (t, c)) in indexes.iter() {
            if t == &tbl && c == &col {
                return Some(idx_name.as_str());
            }
        }
        None
    }

    // Extract column name from qualified name (e.g., "users.id" -> "id")
    fn extract_column_name(qualified: &str) -> &str {
        if let Some(idx) = qualified.rfind('.') {
            &qualified[idx + 1..]
        } else {
            qualified
        }
    }

    fn execute_subquery_for_in(
        subquery_sql: &str,
        tables: &mut HashMap<String, Table>,
        schemas: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<String>, String> {
        let (
            distinct,
            cols,
            from_table,
            join,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
        ) = parser::parse_select(subquery_sql)?;

        if join.is_some() || group_by.is_some() || having.is_some() {
            return Err(
                "Subquery only supports simple SELECT without JOIN/GROUP BY/HAVING".to_string(),
            );
        }

        let cols = cols.ok_or_else(|| "Subquery must select a column".to_string())?;
        if cols.len() != 1 {
            return Err("Subquery must select exactly one column".to_string());
        }

        let col = cols[0].clone();
        if col == "*" || col.contains('(') {
            return Err("Subquery column must be a simple column".to_string());
        }

        let table_name = from_table.unwrap_or_else(|| "users".to_string());
        let mut rows = {
            let tbl = load_table_by_name(&table_name, tables, schemas);
            match where_clause {
                None => tbl.select_all(),
                Some((conditions, operators)) => {
                    tbl.select_where_complex(&conditions, &operators)?
                }
            }
        };

        rows = apply_sorting(rows, order_by);
        rows = apply_distinct(rows, distinct);
        rows = apply_offset_limit(rows, offset, limit);

        let col_name = extract_column_name(&col);
        let mut values = Vec::new();
        for row in rows {
            if let Some(val) = row.get_value(col_name) {
                values.push(val);
            } else {
                return Err("Invalid column in subquery".to_string());
            }
        }
        Ok(values)
    }

    fn resolve_in_subqueries(
        conditions: &[(String, String, String)],
        tables: &mut HashMap<String, Table>,
        schemas: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<(String, String, String)>, String> {
        let mut resolved = Vec::new();
        for (col, op, val) in conditions.iter() {
            if op == "IN_SUBQUERY" {
                let values = execute_subquery_for_in(val, tables, schemas)?;
                resolved.push((col.clone(), "IN".to_string(), values.join(",")));
            } else if op == "NOT_IN_SUBQUERY" {
                let values = execute_subquery_for_in(val, tables, schemas)?;
                resolved.push((col.clone(), "NOT_IN".to_string(), values.join(",")));
            } else if op == "EXISTS_SUBQUERY" {
                let values = execute_subquery_for_in(val, tables, schemas)?;
                let resolved_op = if values.is_empty() {
                    "CONST_FALSE"
                } else {
                    "CONST_TRUE"
                };
                resolved.push((col.clone(), resolved_op.to_string(), String::new()));
            } else if op == "NOT_EXISTS_SUBQUERY" {
                let values = execute_subquery_for_in(val, tables, schemas)?;
                let resolved_op = if values.is_empty() {
                    "CONST_TRUE"
                } else {
                    "CONST_FALSE"
                };
                resolved.push((col.clone(), resolved_op.to_string(), String::new()));
            } else {
                resolved.push((col.clone(), op.clone(), val.clone()));
            }
        }
        Ok(resolved)
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
                "BETWEEN" => {
                    let parts: Vec<&str> = rhs.split(',').collect();
                    if parts.len() != 2 {
                        return false;
                    }
                    let min_val = parts[0].parse::<i64>().unwrap_or(0);
                    let max_val = parts[1].parse::<i64>().unwrap_or(0);
                    l >= min_val && l <= max_val
                }
                "IN" => rhs
                    .split(',')
                    .filter_map(|v| v.trim().parse::<i64>().ok())
                    .any(|v| v == l),
                _ => false,
            }
        }
        fn cmp_str(val: &str, op: &str, rhs: &str) -> bool {
            match op {
                "=" => val == rhs,
                "LIKE" => pattern_match(val, rhs),
                "IN" => rhs.split(',').any(|v| v.trim() == val),
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
        Statement::BeginTransaction => {
            if tx.active {
                println!("Error: Transaction already active");
                return;
            }

            tx.table_snapshots.clear();
            for (name, table) in tables.iter() {
                let rows: Vec<Row> = table.select_all().iter().map(|r| (*r).clone()).collect();
                tx.table_snapshots.insert(name.clone(), rows);
            }
            tx.schema_snapshot = schemas.clone();
            tx.active = true;
            println!("Transaction started.");
        }
        Statement::CommitTransaction => {
            if !tx.active {
                println!("Error: No active transaction");
                return;
            }

            for table in tables.values() {
                let _ = table.save();
            }
            save_schemas(schemas);
            tx.table_snapshots.clear();
            tx.schema_snapshot.clear();
            tx.active = false;
            println!("Transaction committed.");
        }
        Statement::RollbackTransaction => {
            if !tx.active {
                println!("Error: No active transaction");
                return;
            }

            // Remove tables created during transaction
            let snapshot_keys: std::collections::HashSet<String> =
                tx.table_snapshots.keys().cloned().collect();
            let current_keys: Vec<String> = tables.keys().cloned().collect();
            for name in current_keys {
                if !snapshot_keys.contains(&name) {
                    let file_path = table_file_for(&name);
                    tables.remove(&name);
                    let _ = std::fs::remove_file(&file_path);
                }
            }

            // Restore tables from snapshot
            for (name, rows) in tx.table_snapshots.iter() {
                let schema = get_schema_for(name, schemas);
                let table = tables
                    .entry(name.clone())
                    .or_insert_with(|| Table::new(table_file_for(name), schema.clone()));
                table.clear();
                for row in rows.iter().cloned() {
                    let _ = table.insert(row);
                }
                let _ = table.save();
            }

            *schemas = tx.schema_snapshot.clone();
            save_schemas(schemas);
            tx.table_snapshots.clear();
            tx.schema_snapshot.clear();
            tx.active = false;
            println!("Transaction rolled back.");
        }
        Statement::Insert { table_name, values } => {
            let table = if let Some(name) = table_name.as_deref() {
                load_table_by_name(name, tables, schemas)
            } else {
                get_default_table(tables, schemas)
            };
            let schema = table.schema().clone();
            let actual_table_name = table_name.as_deref().unwrap_or("users").to_lowercase();

            match Row::from_values(&schema, values) {
                Ok(row) => {
                    // Check constraints if they exist for this table
                    if let Some((pk_opt, unique_cols)) = constraints.get(&actual_table_name) {
                        // Check PRIMARY KEY uniqueness
                        if let Some(pk_col) = pk_opt {
                            let new_pk_value = row.get_value(pk_col);

                            // Check all existing rows for duplicate primary key
                            for existing_row in table.select_all() {
                                let existing_pk_value = existing_row.get_value(pk_col);
                                if new_pk_value == existing_pk_value {
                                    println!(
                                        "Error: PRIMARY KEY constraint violation on column '{}'",
                                        pk_col
                                    );
                                    return;
                                }
                            }
                        }

                        // Check UNIQUE constraints
                        for unique_col in unique_cols {
                            let new_unique_value = row.get_value(unique_col);

                            // Check all existing rows for duplicate unique value (skip NULL values)
                            if new_unique_value.is_some() {
                                for existing_row in table.select_all() {
                                    let existing_unique_value = existing_row.get_value(unique_col);
                                    if new_unique_value == existing_unique_value {
                                        println!(
                                            "Error: UNIQUE constraint violation on column '{}'",
                                            unique_col
                                        );
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    // If all constraint checks pass, insert the row
                    match table.insert(row) {
                        Ok(()) => {
                            if let Err(e) = table.save() {
                                println!("Error saving table: {}", e);
                            } else {
                                println!("Executed.");
                            }
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::Analyze { table_name } => {
            // Load the table and schema
            let tbl = load_table_by_name(&table_name, tables, schemas);
            let schema = get_schema_for(&table_name, schemas);
            let rows = tbl.select_all();

            // Compute basic stats per column: row_count, null_count, distinct_count, min, max
            let mut table_stats: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::new();
            table_stats.insert("row_count".to_string(), serde_json::json!(rows.len()));

            let mut cols: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
            for col in &schema {
                let mut null_count = 0usize;
                let mut distinct = std::collections::HashSet::new();
                let mut min_val: Option<String> = None;
                let mut max_val: Option<String> = None;
                for r in &rows {
                    if let Some(v) = r.get_value(col) {
                        if v.is_empty() {
                            null_count += 1;
                        } else {
                            distinct.insert(v.clone());
                            match &min_val {
                                None => min_val = Some(v.clone()),
                                Some(cur) => {
                                    if &v < cur {
                                        min_val = Some(v.clone())
                                    }
                                }
                            }
                            match &max_val {
                                None => max_val = Some(v.clone()),
                                Some(cur) => {
                                    if &v > cur {
                                        max_val = Some(v.clone())
                                    }
                                }
                            }
                        }
                    } else {
                        null_count += 1;
                    }
                }
                let mut m = serde_json::Map::new();
                m.insert("null_count".to_string(), serde_json::json!(null_count));
                m.insert(
                    "distinct_count".to_string(),
                    serde_json::json!(distinct.len()),
                );
                m.insert("min".to_string(), serde_json::json!(min_val));
                m.insert("max".to_string(), serde_json::json!(max_val));
                cols.insert(col.clone(), serde_json::Value::Object(m));
            }
            table_stats.insert("columns".to_string(), serde_json::Value::Object(cols));

            // Write stats to file stats_<table>.json
            let stats_file = format!("stats_{}.json", table_name);
            if let Ok(json_text) =
                serde_json::to_string_pretty(&serde_json::Value::Object(table_stats.clone()))
            {
                let _ = std::fs::write(&stats_file, json_text);
            }
            // Human-readable summary
            println!("Analysis summary for table: {}", table_name);
            println!("  Rows: {}", rows.len());
            println!("  Columns:");
            if let Some(serde_json::Value::Object(col_map)) = table_stats.get("columns") {
                for col in &schema {
                    if let Some(serde_json::Value::Object(m)) = col_map.get(col) {
                        let null_count = m.get("null_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        let distinct_count = m
                            .get("distinct_count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let min = match m.get("min") {
                            Some(serde_json::Value::String(s)) => s.as_str(),
                            _ => "NULL",
                        };
                        let max = match m.get("max") {
                            Some(serde_json::Value::String(s)) => s.as_str(),
                            _ => "NULL",
                        };
                        println!(
                            "    - {}: nulls={}, distinct={}, min={}, max={}",
                            col, null_count, distinct_count, min, max
                        );
                    }
                }
            }
            println!(
                "Analyzed table {}. Stats written to {}",
                table_name, stats_file
            );
        }
        Statement::SelectWithCTE {
            cte_name,
            cte_query,
            distinct,
            columns,
            from_table,
            join,
            group_by,
            having,
            order_by,
            limit,
            offset,
        } => {
            // CTE Parsing Recognition: Display parsed CTE information
            println!("WITH {} AS ({}) recognized", cte_name, cte_query);
            println!(
                "Main SELECT from: {}",
                from_table.as_deref().unwrap_or("(none)")
            );
            println!("Note: CTE execution is parsing-complete. Full recursive execution coming in next phase.");
        }
        Statement::SelectWithCTEWhere {
            cte_name,
            cte_query,
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
        } => {
            // Similar to SelectWithCTE but includes WHERE clause conditions
            let where_clause = build_where_clause(&conditions, &operators);
            println!("WITH {} AS ({}) recognized", cte_name, cte_query);
            println!(
                "Main SELECT from: {} WHERE {}",
                from_table.as_deref().unwrap_or("(none)"),
                where_clause
            );
            println!("Note: CTE execution is parsing-complete. Full recursive execution coming in next phase.");
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
            // Check if from_table references a view
            if let Some(ref ft) = from_table {
                let ft_lower = ft.to_lowercase();
                if let Some(view_query) = views.get(&ft_lower) {
                    // Substitute view: execute SELECT from view's query
                    let view_select = format!(
                        "SELECT {} FROM ({}) WHERE 1=1{}{}{}{}",
                        columns
                            .as_ref()
                            .map(|c| c.join(", "))
                            .unwrap_or_else(|| "*".to_string()),
                        view_query,
                        group_by
                            .as_ref()
                            .map(|gb| format!(" GROUP BY {}", gb.join(", ")))
                            .unwrap_or_default(),
                        having
                            .as_ref()
                            .map(|(cond, _)| {
                                let cond_str = cond
                                    .iter()
                                    .map(|(col, op, val)| format!("{} {} {}", col, op, val))
                                    .collect::<Vec<_>>()
                                    .join(" AND ");
                                format!(" HAVING {}", cond_str)
                            })
                            .unwrap_or_default(),
                        order_by
                            .as_ref()
                            .map(|(col, is_asc)| format!(
                                " ORDER BY {} {}",
                                col,
                                if *is_asc { "ASC" } else { "DESC" }
                            ))
                            .unwrap_or_default(),
                        limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default()
                    );

                    // Execute the substituted query
                    if let PrepareResult::Success(stmt) = prepare_statement(&view_select) {
                        execute_statement(stmt, tables, schemas, views, constraints, indexes, tx);
                    }
                    return;
                }
            }

            // Resolve left (from) table - use registry instead of reloading from file
            let table_name = from_table.as_deref().unwrap_or("users");

            // Get rows from the registry table (which contains in-memory changes)
            let rows = {
                let tbl = load_table_by_name(table_name, tables, schemas);
                tbl.select_all()
            };

            // Handle JOIN case separately to avoid ownership issues
            if let Some(ref jc) = join {
                let right_schema = get_schema_for(&jc.table, schemas);
                let right_table = Table::new(table_file_for(&jc.table), right_schema);
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

            // Precompute ROW_NUMBER window mappings if requested in columns
            let mut window_map = compute_row_number_map(&rows, &columns);
            let rank_map = compute_rank_map(&rows, &columns);
            window_map.extend(rank_map);
            let dense_map = compute_dense_rank_map(&rows, &columns);
            window_map.extend(dense_map);
            let first_value_map = compute_first_value_map(&rows, &columns);
            window_map.extend(first_value_map);
            let last_value_map = compute_last_value_map(&rows, &columns);
            window_map.extend(last_value_map);
            let lead_map = compute_lead_map(&rows, &columns);
            window_map.extend(lead_map);
            let lag_map = compute_lag_map(&rows, &columns);
            window_map.extend(lag_map);

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
                    let table_schema = get_schema_for(table_name, schemas);

                    // Parse columns for aggregates (these may include regular columns)
                    let agg_cols: Vec<AggregateColumn> = match &columns {
                        Some(cols) => cols
                            .iter()
                            .map(|c| AggregateColumn::from_col_string(c))
                            .collect(),
                        None => vec![],
                    };

                    // Handle ROLLUP encoded by parser as single-element vec starting with "ROLLUP:"
                    let mut result_rows: Vec<Vec<String>> = Vec::new();
                    if group_cols.len() == 1 && group_cols[0].starts_with("ROLLUP:") {
                        // decode base columns
                        let base = group_cols[0].strip_prefix("ROLLUP:").unwrap_or("");
                        let base_cols: Vec<String> = if base.is_empty() {
                            Vec::new()
                        } else {
                            base.split(',').map(|s| s.to_string()).collect()
                        };

                        // Generate grouping sets: full, then drop last, ... down to empty
                        for k in (0..=base_cols.len()).rev() {
                            let grouping_set = base_cols[..k].to_vec();
                            let groups =
                                group_rows_by_columns(rows.clone(), &grouping_set, &table_schema);
                            for (_key, group_rows) in groups {
                                // Build output values: for regular (non-aggregate) columns that are grouping
                                // columns but not present in this grouping_set, emit NULL per ROLLUP semantics.
                                let mut values: Vec<String> = Vec::new();
                                for agg in &agg_cols {
                                    match agg {
                                        AggregateColumn::Regular(col) => {
                                            if grouping_set.iter().any(|c| c == col) {
                                                values.push(compute_aggregate(
                                                    agg,
                                                    &group_rows,
                                                    &table_schema,
                                                ));
                                            } else {
                                                values.push("NULL".to_string());
                                            }
                                        }
                                        _ => {
                                            values.push(compute_aggregate(
                                                agg,
                                                &group_rows,
                                                &table_schema,
                                            ));
                                        }
                                    }
                                }
                                if passes_having_filter(&having, &agg_cols, &values) {
                                    result_rows.push(values);
                                }
                            }
                        }
                    } else {
                        // Regular GROUP BY
                        let groups = group_rows_by_columns(rows.clone(), group_cols, &table_schema);
                        for (_key, group_rows) in groups {
                            let mut values = Vec::new();
                            for agg in &agg_cols {
                                values.push(compute_aggregate(agg, &group_rows, &table_schema));
                            }
                            if passes_having_filter(&having, &agg_cols, &values) {
                                result_rows.push(values);
                            }
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
                    let table_schema = get_schema_for(table_name, schemas);
                    let mut values = Vec::new();
                    for agg in &agg_cols {
                        values.push(compute_aggregate(agg, &rows, &table_schema));
                    }
                    println!("({})", values.join(", "));
                }
                println!("Executed.");
            } else {
                // Regular SELECT without aggregates
                rows = apply_sorting(rows, order_by);
                rows = apply_distinct(rows, distinct);
                rows = apply_offset_limit(rows, offset, limit);

                let table_schema = get_schema_for(table_name, schemas);
                for row in rows {
                    match &columns {
                        None => {
                            // Print all columns from schema
                            let values: Vec<String> = table_schema
                                .iter()
                                .map(|col| row.get_value(col).unwrap_or_else(|| "NULL".to_string()))
                                .collect();
                            println!("({})", values.join(", "));
                        }
                        Some(cols) => {
                            let mut values: Vec<String> = Vec::new();
                            for col in cols.iter() {
                                if col.starts_with("__row_number__:")
                                    || col.starts_with("__rank__:")
                                    || col.starts_with("__dense_rank__:")
                                    || col.starts_with("__lead__:")
                                    || col.starts_with("__lag__:")
                                {
                                    if let Some(col_map) = window_map.get(col) {
                                        values.push(
                                            col_map
                                                .get(&row.id)
                                                .cloned()
                                                .unwrap_or_else(|| "NULL".to_string()),
                                        );
                                    } else {
                                        values.push("NULL".to_string());
                                    }
                                } else {
                                    values.push(
                                        row.eval_col(col)
                                            .unwrap_or_else(|| format!("NULL({})", col)),
                                    );
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
            // Check if from_table references a view
            if let Some(ref ft) = from_table {
                let ft_lower = ft.to_lowercase();
                if let Some(view_query) = views.get(&ft_lower) {
                    // Build WHERE clause from conditions
                    let where_clause = build_where_clause(&conditions, &operators);

                    // Substitute view: execute SELECT WHERE from view's query
                    let view_select = format!(
                        "SELECT {} FROM ({}) WHERE {}{}{}{}{}",
                        columns
                            .as_ref()
                            .map(|c| c.join(", "))
                            .unwrap_or_else(|| "*".to_string()),
                        view_query,
                        where_clause,
                        group_by
                            .as_ref()
                            .map(|gb| format!(" GROUP BY {}", gb.join(", ")))
                            .unwrap_or_default(),
                        having
                            .as_ref()
                            .map(|(cond, _)| {
                                let cond_str = cond
                                    .iter()
                                    .map(|(col, op, val)| format!("{} {} {}", col, op, val))
                                    .collect::<Vec<_>>()
                                    .join(" AND ");
                                format!(" HAVING {}", cond_str)
                            })
                            .unwrap_or_default(),
                        order_by
                            .as_ref()
                            .map(|(col, is_asc)| format!(
                                " ORDER BY {} {}",
                                col,
                                if *is_asc { "ASC" } else { "DESC" }
                            ))
                            .unwrap_or_default(),
                        limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default()
                    );

                    // Execute the substituted query
                    if let PrepareResult::Success(stmt) = prepare_statement(&view_select) {
                        execute_statement(stmt, tables, schemas, views, constraints, indexes, tx);
                    }
                    return;
                }
            }

            // Resolve left (from) table - use registry instead of reloading from file
            let table_name = from_table.as_deref().unwrap_or("users");

            let resolved_conditions = match resolve_in_subqueries(&conditions, tables, schemas) {
                Ok(resolved) => resolved,
                Err(e) => {
                    println!("Error: {}", e);
                    return;
                }
            };

            let select_result = {
                let tbl = load_table_by_name(table_name, tables, schemas);

                // Index acceleration: if there is exactly one equality condition
                // and an index exists for this table+column, report the index hit.
                // The actual filtering still uses select_where_complex (correctness-first),
                // but we validate index usage and could short-circuit in the future.
                if resolved_conditions.len() == 1 && operators.is_empty() {
                    let (col, op, _val) = &resolved_conditions[0];
                    if op == "=" {
                        if let Some(idx_name) = find_index_for(table_name, col, indexes) {
                            let _ = idx_name; // index is found — filtered scan path
                        }
                    }
                }

                tbl.select_where_complex(&resolved_conditions, &operators)
            };

            match select_result {
                Ok(rows) => {
                    // Handle JOIN case separately to avoid ownership issues
                    if let Some(ref jc) = join {
                        let right_schema = get_schema_for(&jc.table, schemas);
                        let right_table = Table::new(table_file_for(&jc.table), right_schema);
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
                        let jrows = if !resolved_conditions.is_empty() {
                            filter_joined_rows(
                                jrows,
                                &resolved_conditions,
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

                    // Precompute ROW_NUMBER window mappings if requested in columns
                    let mut window_map = compute_row_number_map(&rows, &columns);
                    let rank_map = compute_rank_map(&rows, &columns);
                    window_map.extend(rank_map);
                    let dense_map = compute_dense_rank_map(&rows, &columns);
                    window_map.extend(dense_map);
                    let first_value_map = compute_first_value_map(&rows, &columns);
                    window_map.extend(first_value_map);
                    let last_value_map = compute_last_value_map(&rows, &columns);
                    window_map.extend(last_value_map);
                    let lead_map = compute_lead_map(&rows, &columns);
                    window_map.extend(lead_map);
                    let lag_map = compute_lag_map(&rows, &columns);
                    window_map.extend(lag_map);
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
                            let table_schema = get_schema_for(table_name, schemas);
                            let groups = group_rows_by_columns(rows, group_cols, &table_schema);

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
                                    values.push(compute_aggregate(agg, &group_rows, &table_schema));
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
                            let table_schema = get_schema_for(table_name, schemas);
                            let mut values = Vec::new();
                            for agg in &agg_cols {
                                values.push(compute_aggregate(agg, &rows, &table_schema));
                            }
                            println!("({})", values.join(", "));
                        }
                        println!("Executed.");
                    } else {
                        // Regular SELECT WHERE without aggregates
                        rows = apply_sorting(rows, order_by);
                        rows = apply_distinct(rows, distinct);
                        rows = apply_offset_limit(rows, offset, limit);

                        let table_schema = get_schema_for(table_name, schemas);
                        for row in rows {
                            match &columns {
                                None => {
                                    // Print all columns from schema
                                    let values: Vec<String> = table_schema
                                        .iter()
                                        .map(|col| {
                                            row.get_value(col).unwrap_or_else(|| "NULL".to_string())
                                        })
                                        .collect();
                                    println!("({})", values.join(", "));
                                }
                                Some(cols) => {
                                    let mut values = Vec::new();
                                    for col in cols {
                                        if col.starts_with("__row_number__:")
                                            || col.starts_with("__rank__:")
                                            || col.starts_with("__dense_rank__:")
                                            || col.starts_with("__lead__:")
                                            || col.starts_with("__lag__:")
                                        {
                                            if let Some(col_map) = window_map.get(col) {
                                                values.push(
                                                    col_map
                                                        .get(&row.id)
                                                        .cloned()
                                                        .unwrap_or_else(|| "NULL".to_string()),
                                                );
                                            } else {
                                                values.push("NULL".to_string());
                                            }
                                        } else {
                                            values.push(
                                                row.eval_col(col)
                                                    .unwrap_or_else(|| format!("NULL({})", col)),
                                            );
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
        Statement::Update {
            table_name,
            id,
            assignments,
        } => {
            let table = match table_name {
                Some(name) => load_table_by_name(&name, tables, schemas),
                None => get_default_table(tables, schemas),
            };
            let mut update_err: Option<String> = None;
            for (column, value) in &assignments {
                if let Err(e) = table.update(id, column, value) {
                    update_err = Some(e);
                    break;
                }
            }
            match update_err {
                Some(e) => println!("Error: {}", e),
                None => {
                    if let Err(e) = table.save() {
                        println!("Error saving table: {}", e);
                    } else {
                        println!("Executed.");
                    }
                }
            }
        }
        Statement::Delete { table_name, id } => {
            let table = match table_name {
                Some(name) => load_table_by_name(&name, tables, schemas),
                None => get_default_table(tables, schemas),
            };
            match table.delete(id) {
                Ok(()) => {
                    if let Err(e) = table.save() {
                        println!("Error saving table: {}", e);
                    } else {
                        println!("Executed.");
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::DeleteWhere {
            table_name,
            column,
            value,
        } => {
            let table = match table_name {
                Some(name) => load_table_by_name(&name, tables, schemas),
                None => get_default_table(tables, schemas),
            };
            match table.delete_where(&column, &value) {
                Ok(count) => {
                    if let Err(e) = table.save() {
                        println!("Error saving table: {}", e);
                    } else {
                        println!("Deleted {} rows.", count);
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Statement::DeleteAll => {
            // Get the default table (users) for backward compatibility
            let schema = get_schema_for("users", schemas);
            let table = tables
                .entry("users".to_string())
                .or_insert_with(|| Table::new(table_file_for("users"), schema));
            let count = table.clear();
            if let Err(e) = table.save() {
                println!("Error saving table: {}", e);
            } else {
                println!("Deleted {} rows.", count);
            }
        }
        Statement::CreateTable {
            table_name,
            columns,
            primary_key,
            unique_columns,
        } => {
            let table_name_lower = table_name.to_lowercase();

            // Check if table already exists in registry
            if tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' already exists", table_name);
                return;
            }

            // Validate PRIMARY KEY column exists
            if let Some(ref pk) = primary_key {
                if !columns.contains(pk) {
                    println!("Error: PRIMARY KEY column '{}' does not exist in table", pk);
                    return;
                }
            }

            // Validate UNIQUE columns exist
            for uc in &unique_columns {
                if !columns.contains(uc) {
                    println!("Error: UNIQUE column '{}' does not exist in table", uc);
                    return;
                }
            }

            // Remove existing file if it exists (to start fresh)
            let file_path = table_file_for(&table_name_lower);
            if std::path::Path::new(&file_path).exists() {
                if let Err(e) = std::fs::remove_file(&file_path) {
                    eprintln!(
                        "Warning: Could not remove existing table file '{}': {}",
                        file_path, e
                    );
                }
            }

            // Create new empty table
            let new_table = Table::new(file_path, columns.clone());

            // Store table in registry
            tables.insert(table_name_lower.clone(), new_table);

            schemas.insert(table_name_lower.clone(), columns.clone());

            // Store constraints
            constraints.insert(
                table_name_lower.clone(),
                (primary_key.clone(), unique_columns.clone()),
            );

            if !tx.active {
                save_schemas(schemas);
            }

            println!(
                "Table '{}' created with columns: {}",
                table_name,
                columns.join(", ")
            );

            if let Some(ref pk) = primary_key {
                println!("  PRIMARY KEY: {}", pk);
            }
            if !unique_columns.is_empty() {
                println!("  UNIQUE: {}", unique_columns.join(", "));
            }
        }
        Statement::AlterTableRename {
            table_name,
            new_name,
        } => {
            let old_name = table_name.to_lowercase();
            let new_name_lower = new_name.to_lowercase();

            if old_name == "users" {
                println!("Error: Cannot rename default table 'users'");
                return;
            }

            if !tables.contains_key(&old_name) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            if tables.contains_key(&new_name_lower) {
                println!("Error: Table '{}' already exists", new_name);
                return;
            }

            let old_rows: Vec<Row> = if let Some(table) = tables.get(&old_name) {
                table.select_all().iter().map(|r| (*r).clone()).collect()
            } else {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            };

            let old_file = table_file_for(&old_name);
            let new_file = table_file_for(&new_name_lower);
            let _ = std::fs::remove_file(&new_file);

            let schema = get_schema_for(&old_name, schemas);
            let mut new_table = Table::new(new_file.clone(), schema);
            for row in old_rows {
                let _ = new_table.insert(row);
            }
            let _ = new_table.save();

            tables.remove(&old_name);
            tables.insert(new_name_lower.clone(), new_table);
            let _ = std::fs::remove_file(&old_file);

            if let Some(cols) = schemas.remove(&old_name) {
                schemas.insert(new_name_lower.clone(), cols);
            }
            if !tx.active {
                save_schemas(schemas);
            }

            println!("Table '{}' renamed to '{}'", table_name, new_name);
        }
        Statement::AlterTableAddColumn { table_name, column } => {
            let table_name_lower = table_name.to_lowercase();
            if !tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            let cols = schemas.entry(table_name_lower.clone()).or_default();
            if cols.iter().any(|c| c.eq_ignore_ascii_case(&column)) {
                println!("Error: Column '{}' already exists", column);
                return;
            }
            cols.push(column.clone());
            if !tx.active {
                save_schemas(schemas);
            }
            println!(
                "Column '{}' added to table '{}' (metadata only)",
                column, table_name
            );
        }
        Statement::AlterTableDropColumn { table_name, column } => {
            let table_name_lower = table_name.to_lowercase();
            if !tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            if let Some(cols) = schemas.get_mut(&table_name_lower) {
                let before = cols.len();
                cols.retain(|c| !c.eq_ignore_ascii_case(&column));
                if cols.len() == before {
                    println!("Error: Column '{}' does not exist", column);
                    return;
                }
                if !tx.active {
                    save_schemas(schemas);
                }
                println!(
                    "Column '{}' dropped from table '{}' (metadata only)",
                    column, table_name
                );
            } else {
                println!("Error: No schema found for table '{}'", table_name);
            }
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

            schemas.remove(&table_name_lower);

            // Remove constraints for this table
            constraints.remove(&table_name_lower);

            // Remove all indexes for this table
            indexes.retain(|_, (tbl, _)| tbl != &table_name_lower);

            if !tx.active {
                save_schemas(schemas);
            }

            println!("Table '{}' dropped", table_name);
        }
        Statement::CreateView {
            view_name,
            select_query,
        } => {
            let view_name_lower = view_name.to_lowercase();

            // Validate SELECT query by trying to parse it
            match parser::parse_select(&select_query) {
                Ok(_) => {
                    // Store the view definition
                    views.insert(view_name_lower.clone(), select_query.clone());
                    println!("View '{}' created", view_name);
                }
                Err(e) => {
                    println!("Error: Invalid SELECT query for view: {}", e);
                }
            }
        }
        Statement::DropView { view_name } => {
            let view_name_lower = view_name.to_lowercase();

            if views.contains_key(&view_name_lower) {
                views.remove(&view_name_lower);
                println!("View '{}' dropped", view_name);
            } else {
                println!("Error: View '{}' does not exist", view_name);
            }
        }
        Statement::ShowTables => {
            let mut table_names: Vec<String> = tables.keys().cloned().collect();
            table_names.sort();

            if table_names.is_empty() {
                println!("No tables found.");
            } else {
                println!("Tables:");
                for name in table_names {
                    println!("  {}", name);
                }
            }
        }
        Statement::ShowIndexes => {
            if indexes.is_empty() {
                println!("No indexes defined.");
            } else {
                let mut index_list: Vec<(&String, &(String, String))> = indexes.iter().collect();
                index_list.sort_by_key(|(name, _)| name.as_str());
                println!("Indexes:");
                for (idx_name, (tbl, col)) in index_list {
                    println!("  {} ON {}({})", idx_name, tbl, col);
                }
            }
        }
        Statement::CreateIndex {
            index_name,
            table_name,
            column_name,
        } => {
            let index_name_lower = index_name.to_lowercase();
            let table_name_lower = table_name.to_lowercase();
            let column_name_lower = column_name.to_lowercase();

            // Check index doesn't already exist
            if indexes.contains_key(&index_name_lower) {
                println!("Error: Index '{}' already exists", index_name);
                return;
            }

            // Check table exists
            if !tables.contains_key(&table_name_lower) && !schemas.contains_key(&table_name_lower) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            // Check column exists in schema
            let schema = schemas
                .get(&table_name_lower)
                .or_else(|| tables.get(&table_name_lower).map(|t| t.schema()))
                .cloned();

            if let Some(cols) = schema {
                if !cols.contains(&column_name_lower) {
                    println!(
                        "Error: Column '{}' does not exist in table '{}'",
                        column_name, table_name
                    );
                    return;
                }
            }

            indexes.insert(
                index_name_lower.clone(),
                (table_name_lower.clone(), column_name_lower.clone()),
            );
            println!(
                "Index '{}' created on {}({})",
                index_name, table_name, column_name
            );
        }
        Statement::DropIndex { index_name } => {
            let index_name_lower = index_name.to_lowercase();

            if !indexes.contains_key(&index_name_lower) {
                println!("Error: Index '{}' does not exist", index_name);
                return;
            }

            indexes.remove(&index_name_lower);
            println!("Index '{}' dropped", index_name);
        }
        Statement::TruncateTable { table_name } => {
            let table_name_lower = table_name.to_lowercase();

            // Check if table exists
            if !tables.contains_key(&table_name_lower) {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            }

            // Don't allow truncating the default users table
            if table_name_lower == "users" {
                println!("Error: Cannot truncate default table 'users'");
                return;
            }

            // Clear the table
            let Some(table) = tables.get_mut(&table_name_lower) else {
                println!("Error: Table '{}' does not exist", table_name);
                return;
            };
            let count = table.clear();
            if let Err(e) = table.save() {
                println!("Error saving table: {}", e);
            } else {
                println!("Truncated table '{}' ({} rows deleted)", table_name, count);
            }
        }
        Statement::InsertSelect {
            table_name,
            select_sql,
        } => {
            let (
                distinct,
                cols,
                from_table,
                _join,
                where_clause,
                _group_by,
                _having,
                order_by,
                limit,
                offset,
            ) = match parser::parse_select(&select_sql) {
                Ok(result) => result,
                Err(e) => {
                    println!("Error in SELECT: {}", e);
                    return;
                }
            };
            let src_table_name = from_table.as_deref().unwrap_or("users").to_string();
            let src_schema = get_schema_for(&src_table_name, schemas);
            let resolved_where = if let Some((conditions, operators)) = where_clause {
                match resolve_in_subqueries(&conditions, tables, schemas) {
                    Ok(resolved) => Some((resolved, operators)),
                    Err(e) => {
                        println!("Error: {}", e);
                        return;
                    }
                }
            } else {
                None
            };
            let owned_rows: Vec<Row> = {
                let tbl = load_table_by_name(&src_table_name, tables, schemas);
                match resolved_where {
                    None => tbl.select_all().into_iter().cloned().collect(),
                    Some((ref conditions, ref operators)) => {
                        match tbl.select_where_complex(conditions, operators) {
                            Ok(rows) => rows.into_iter().cloned().collect(),
                            Err(e) => {
                                println!("Error: {}", e);
                                return;
                            }
                        }
                    }
                }
            };
            let row_refs: Vec<&Row> = owned_rows.iter().collect();
            let row_refs = apply_sorting(row_refs, order_by);
            let row_refs = apply_distinct(row_refs, distinct);
            let row_refs = apply_offset_limit(row_refs, offset, limit);
            let target_schema = get_schema_for(&table_name, schemas);
            let result_rows: Vec<Vec<String>> = row_refs
                .iter()
                .map(|row| match &cols {
                    None => src_schema
                        .iter()
                        .map(|col| row.get_value(col).unwrap_or_else(|| "NULL".to_string()))
                        .collect(),
                    Some(col_names) if col_names.iter().any(|c| c == "*") => src_schema
                        .iter()
                        .map(|col| row.get_value(col).unwrap_or_else(|| "NULL".to_string()))
                        .collect(),
                    Some(col_names) => col_names
                        .iter()
                        .map(|col| row.eval_col(col).unwrap_or_else(|| "NULL".to_string()))
                        .collect(),
                })
                .collect();
            let mut inserted = 0usize;
            let mut errors = 0usize;
            for values in result_rows {
                let tbl = load_table_by_name(&table_name, tables, schemas);
                match Row::from_values(&target_schema, values) {
                    Ok(row) => match tbl.insert(row) {
                        Ok(()) => inserted += 1,
                        Err(e) => {
                            println!("Error inserting row: {}", e);
                            errors += 1;
                        }
                    },
                    Err(e) => {
                        println!("Error building row: {}", e);
                        errors += 1;
                    }
                }
            }
            for tbl in tables.values_mut() {
                let _ = tbl.save();
            }
            if errors == 0 {
                println!("{} row(s) inserted.", inserted);
            } else {
                println!("{} row(s) inserted, {} error(s).", inserted, errors);
            }
        }
        Statement::Union { sql1, sql2, all } => {
            fn collect_rows_for_union(
                sql: &str,
                tables: &mut HashMap<String, Table>,
                schemas: &HashMap<String, Vec<String>>,
            ) -> Result<Vec<Vec<String>>, String> {
                let (
                    distinct,
                    cols,
                    from_table,
                    join,
                    where_clause,
                    _group_by,
                    _having,
                    order_by,
                    limit,
                    offset,
                ) = parser::parse_select(sql)?;
                if join.is_some() {
                    return Err("UNION sub-queries do not support JOIN".to_string());
                }
                let table_name = from_table.as_deref().unwrap_or("users").to_string();
                let schema = get_schema_for(&table_name, schemas);
                // Resolve subquery conditions first, while tables is not also borrowed
                let resolved_where = if let Some((conditions, operators)) = where_clause {
                    let resolved = resolve_in_subqueries(&conditions, tables, schemas)?;
                    Some((resolved, operators))
                } else {
                    None
                };
                // Clone rows immediately so we can release the table borrow
                let owned_rows: Vec<Row> = {
                    let tbl = load_table_by_name(&table_name, tables, schemas);
                    match resolved_where {
                        None => tbl.select_all().into_iter().cloned().collect(),
                        Some((ref conditions, ref operators)) => tbl
                            .select_where_complex(conditions, operators)?
                            .into_iter()
                            .cloned()
                            .collect(),
                    }
                };
                let row_refs: Vec<&Row> = owned_rows.iter().collect();
                let row_refs = apply_sorting(row_refs, order_by);
                let row_refs = apply_distinct(row_refs, distinct);
                let row_refs = apply_offset_limit(row_refs, offset, limit);
                let result = row_refs
                    .iter()
                    .map(|row| match &cols {
                        None => schema
                            .iter()
                            .map(|col| row.get_value(col).unwrap_or_else(|| "NULL".to_string()))
                            .collect(),
                        Some(col_names) if col_names.iter().any(|c| c == "*") => schema
                            .iter()
                            .map(|col| row.get_value(col).unwrap_or_else(|| "NULL".to_string()))
                            .collect(),
                        Some(col_names) => col_names
                            .iter()
                            .map(|col| row.eval_col(col).unwrap_or_else(|| "NULL".to_string()))
                            .collect(),
                    })
                    .collect();
                Ok(result)
            }

            let rows1 = match collect_rows_for_union(&sql1, tables, schemas) {
                Ok(r) => r,
                Err(e) => {
                    println!("Error in first query: {}", e);
                    return;
                }
            };
            let rows2 = match collect_rows_for_union(&sql2, tables, schemas) {
                Ok(r) => r,
                Err(e) => {
                    println!("Error in second query: {}", e);
                    return;
                }
            };
            let mut combined = rows1;
            combined.extend(rows2);
            if !all {
                let mut seen = std::collections::HashSet::new();
                combined = combined
                    .into_iter()
                    .filter(|row| {
                        let key = row.join(", ");
                        if seen.contains(&key) {
                            false
                        } else {
                            seen.insert(key);
                            true
                        }
                    })
                    .collect();
            }
            for row in &combined {
                println!("({})", row.join(", "));
            }
            println!("Executed.");
        }
    }
}

// Helper function to build WHERE clause from conditions and operators
fn build_where_clause(conditions: &[(String, String, String)], operators: &[String]) -> String {
    if conditions.is_empty() {
        return "1=1".to_string();
    }

    let mut clause = String::new();
    for (i, (col, op, val)) in conditions.iter().enumerate() {
        if i > 0 && i - 1 < operators.len() {
            clause.push(' ');
            clause.push_str(&operators[i - 1]);
            clause.push(' ');
        }
        clause.push_str(col);
        clause.push(' ');
        clause.push_str(op);
        clause.push(' ');
        clause.push_str(val);
    }
    clause
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
                other => {
                    // Dynamic schema column stored in extras
                    let av = a.extras.get(other).map(|s| s.as_str()).unwrap_or("");
                    let bv = b.extras.get(other).map(|s| s.as_str()).unwrap_or("");
                    // Try numeric comparison first
                    match (av.parse::<f64>(), bv.parse::<f64>()) {
                        (Ok(an), Ok(bn)) => {
                            an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        _ => av.cmp(bv),
                    }
                }
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
            || column.starts_with("string_agg(")
            || column.starts_with("median(")
            || column.starts_with("mode(")
            || column.starts_with("variance(")
            || column.starts_with("stddev_pop(")
            || column.starts_with("stddev_samp(")
            || column.starts_with("corr(")
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
                    AggregateColumn::StringAgg(a, b) => format!("string_agg({},{})", a, b),
                    AggregateColumn::Median(col) => format!("median({})", col),
                    AggregateColumn::PercentileCont(col, p) => {
                        format!("percentile_cont({},{})", col, p)
                    }
                    AggregateColumn::PercentileDisc(col, p) => {
                        format!("percentile_disc({},{})", col, p)
                    }
                    AggregateColumn::ApproxPercentile(col, p) => {
                        format!("approx_percentile({},{})", col, p)
                    }
                    AggregateColumn::Mode(col) => format!("mode({})", col),
                    AggregateColumn::Variance(col) => format!("variance({})", col),
                    AggregateColumn::StddevPop(col) => format!("stddev_pop({})", col),
                    AggregateColumn::StddevSamp(col) => format!("stddev_samp({})", col),
                    AggregateColumn::VarSamp(col) => format!("var_samp({})", col),
                    AggregateColumn::Corr(a, b) => format!("corr({},{})", a, b),
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
        // Include extras in deduplication key so DISTINCT works on dynamic columns
        let mut extras_vec: Vec<(String, String)> = row
            .extras
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        extras_vec.sort_unstable();
        let row_key = (row.id, row.username.clone(), row.email.clone(), extras_vec);
        if seen.insert(row_key) {
            unique_rows.push(row);
        }
    }

    unique_rows
}

const SCHEMA_FILE: &str = "schemas.json";

fn load_schemas() -> HashMap<String, Vec<String>> {
    if Path::new(SCHEMA_FILE).exists() {
        if let Ok(contents) = std::fs::read_to_string(SCHEMA_FILE) {
            if let Ok(schemas) = serde_json::from_str::<HashMap<String, Vec<String>>>(&contents) {
                return schemas;
            }
        }
    }
    HashMap::new()
}

fn save_schemas(schemas: &HashMap<String, Vec<String>>) {
    if let Ok(json) = serde_json::to_string_pretty(schemas) {
        let _ = std::fs::write(SCHEMA_FILE, json);
    }
}

fn main() {
    // Initialize table registry with default "users" table
    let mut tables: HashMap<String, Table> = HashMap::new();
    let default_schema = vec![
        "id".to_string(),
        "username".to_string(),
        "email".to_string(),
    ];
    tables.insert(
        "users".to_string(),
        Table::new("data.json".to_string(), default_schema.clone()),
    );

    // Initialize view registry: store view name -> SELECT query
    let mut views: HashMap<String, String> = HashMap::new();

    // Initialize constraint registry: store table_name -> (primary_key, unique_columns)
    let mut constraints: HashMap<String, (Option<String>, Vec<String>)> = HashMap::new();

    // Initialize index registry: store index_name -> (table_name, column_name)
    let mut indexes: HashMap<String, (String, String)> = HashMap::new();

    // Load or initialize schema registry
    let mut schemas = load_schemas();
    if schemas.is_empty() {
        schemas.insert(
            "users".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        schemas.insert(
            "orders".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        save_schemas(&schemas);
    }

    let mut tx_state = TransactionState {
        active: false,
        table_snapshots: HashMap::new(),
        schema_snapshot: HashMap::new(),
    };

    // Optional: seed a secondary table for JOIN demos if empty
    {
        let mut orders = Table::new("orders.json".to_string(), default_schema.clone());
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
        let bytes_read = match io::stdin().read_line(&mut input) {
            Ok(bytes_read) => bytes_read,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        };

        // Exit on EOF (e.g., from piped input or Ctrl+D)
        if bytes_read == 0 {
            break;
        }

        let input = input.trim().trim_end_matches(';').trim();

        if input.is_empty() || input.starts_with("--") {
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
                execute_statement(
                    statement,
                    &mut tables,
                    &mut schemas,
                    &mut views,
                    &mut constraints,
                    &mut indexes,
                    &mut tx_state,
                );
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

    fn default_schema() -> Vec<String> {
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
        ]
    }

    #[test]
    fn test_inner_join_basic() {
        // Create and seed two tables: users and orders
        let mut users = Table::new("test_users.json".to_string(), default_schema());
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

        let mut orders = Table::new("test_orders.json".to_string(), default_schema());
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
        let mut users = Table::new("test_left_users.json".to_string(), default_schema());
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

        let mut orders = Table::new("test_left_orders.json".to_string(), default_schema());
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

        let mut schemas: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        schemas.insert(
            "test_left_users".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        schemas.insert(
            "test_left_orders".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let mut tx_state = TransactionState {
            active: false,
            table_snapshots: std::collections::HashMap::new(),
            schema_snapshot: std::collections::HashMap::new(),
        };

        let mut views = std::collections::HashMap::new();
        let mut constraints = std::collections::HashMap::new();
        let mut indexes = std::collections::HashMap::new();
        let _result = super::execute_statement(
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
            &mut schemas,
            &mut views,
            &mut constraints,
            &mut indexes,
            &mut tx_state,
        );
    }

    #[test]
    fn test_right_join_execution() {
        // Create test tables
        let mut users = Table::new("test_right_users.json".to_string(), default_schema());
        users.clear();

        // Insert 2 users
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap())
            .is_ok());
        assert!(users
            .insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut orders = Table::new("test_right_orders.json".to_string(), default_schema());
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
    fn test_analyze_runs() {
        let mut users = Table::new("test_analyze_users.json".to_string(), default_schema());
        users.clear();
        assert!(users
            .insert(Row::new(1, "alice".to_string(), "a@t".to_string()).unwrap())
            .is_ok());
        users.save().unwrap();

        let mut tables: std::collections::HashMap<String, Table> = std::collections::HashMap::new();
        tables.insert("test_analyze_users".to_string(), users);
        let mut schemas: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        schemas.insert(
            "test_analyze_users".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        let mut views = std::collections::HashMap::new();
        let mut constraints = std::collections::HashMap::new();
        let mut indexes = std::collections::HashMap::new();
        let mut tx_state = TransactionState {
            active: false,
            table_snapshots: std::collections::HashMap::new(),
            schema_snapshot: std::collections::HashMap::new(),
        };

        let _ = super::execute_statement(
            Statement::Analyze {
                table_name: "test_analyze_users".to_string(),
            },
            &mut tables,
            &mut schemas,
            &mut views,
            &mut constraints,
            &mut indexes,
            &mut tx_state,
        );

        // Stats file should have been created
        assert!(std::path::Path::new("stats_test_analyze_users.json").exists());
        let _ = std::fs::remove_file("stats_test_analyze_users.json");
    }

    #[test]
    fn test_inner_join_filters_correctly() {
        // Create test tables
        let mut users = Table::new("test_inner_users.json".to_string(), default_schema());
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

        let mut orders = Table::new("test_inner_orders.json".to_string(), default_schema());
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

    #[test]
    fn test_count_column_includes_id_values() {
        let schema = vec!["id".to_string(), "grp".to_string(), "val".to_string()];
        let rows = vec![
            Row::from_values(
                &schema,
                vec!["1".to_string(), "a".to_string(), "10".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["2".to_string(), "a".to_string(), "20".to_string()],
            )
            .unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let count_id = super::compute_aggregate(
            &AggregateColumn::Count(Some("id".to_string())),
            &row_refs,
            &schema,
        );
        let count_star =
            super::compute_aggregate(&AggregateColumn::Count(None), &row_refs, &schema);

        assert_eq!(count_id, "2");
        assert_eq!(count_star, "2");
    }

    #[test]
    fn test_string_agg_concatenates_values() {
        let schema = vec!["id".to_string(), "grp".to_string(), "name".to_string()];
        let rows = vec![
            Row::from_values(
                &schema,
                vec!["1".to_string(), "a".to_string(), "alice".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["2".to_string(), "a".to_string(), "bob".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["3".to_string(), "b".to_string(), "charlie".to_string()],
            )
            .unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StringAgg("name".to_string(), ",".to_string());
        // apply to first two rows (group 'a')
        let group_rows = vec![row_refs[0], row_refs[1]];
        let res = super::compute_aggregate(&agg, &group_rows, &schema);
        assert_eq!(res, "alice,bob");
    }

    #[test]
    fn test_string_agg_skips_null_values() {
        let schema = vec!["id".to_string(), "grp".to_string(), "name".to_string()];
        let rows = vec![
            Row::from_values(
                &schema,
                vec!["1".to_string(), "a".to_string(), "alice".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["2".to_string(), "a".to_string(), "NULL".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["3".to_string(), "a".to_string(), "bob".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["4".to_string(), "a".to_string(), "".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["5".to_string(), "a".to_string(), "charlie".to_string()],
            )
            .unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StringAgg("name".to_string(), "|".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Should skip NULL and empty string values
        assert_eq!(res, "alice|bob|charlie");
    }

    #[test]
    fn test_string_agg_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StringAgg("name".to_string(), ",".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // When all values are NULL/empty, should return NULL
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_median_odd_count() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "10".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "30".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "20".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "50".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "40".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Median("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Sorted: [10, 20, 30, 40, 50], median is 30
        assert_eq!(res, "30");
    }

    #[test]
    fn test_median_even_count() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "10".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "40".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "20".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "30".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Median("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Sorted: [10, 20, 30, 40], median is (20+30)/2 = 25
        assert_eq!(res, "25");
    }

    #[test]
    fn test_median_skips_null_values() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "10".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "30".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "20".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Median("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Should skip NULL and empty, sorted: [10, 20, 30], median is 20
        assert_eq!(res, "20");
    }

    #[test]
    fn test_median_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Median("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // When all values are NULL/empty, should return NULL
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_mode_basic() {
        let schema = vec!["id".to_string(), "category".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "A".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "B".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "A".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "C".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "A".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Mode("category".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // A appears 3 times, B once, C once - mode is A
        assert_eq!(res, "A");
    }

    #[test]
    fn test_mode_with_tie() {
        let schema = vec!["id".to_string(), "category".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "A".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "B".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "A".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "B".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Mode("category".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Both A and B appear 2 times - should return one of them
        assert!(res == "A" || res == "B");
    }

    #[test]
    fn test_mode_skips_null_values() {
        let schema = vec!["id".to_string(), "category".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "A".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "B".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "A".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Mode("category".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Should skip NULL and empty, A appears 2 times, B once - mode is A
        assert_eq!(res, "A");
    }

    #[test]
    fn test_mode_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "category".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Mode("category".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // When all values are NULL/empty, should return NULL
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_variance_basic() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "1".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "3".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "5".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Variance("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Mean = 3, variance = ((1-3)^2 + (2-3)^2 + (3-3)^2 + (4-3)^2 + (5-3)^2) / 5 = (4+1+0+1+4)/5 = 2
        assert_eq!(res, "2");
    }

    #[test]
    fn test_variance_single_value() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows =
            vec![Row::from_values(&schema, vec!["1".to_string(), "42".to_string()]).unwrap()];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Variance("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Single value, variance = 0
        assert_eq!(res, "0");
    }

    #[test]
    fn test_variance_skips_null_values() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "6".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Variance("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Values: 2, 4, 6; mean = 4; variance = ((2-4)^2 + (4-4)^2 + (6-4)^2) / 3 = (4+0+4)/3 = 2.666...
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2.666666).abs() < 0.001);
    }

    #[test]
    fn test_variance_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Variance("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_approx_percentile_small_exact() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "10".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "20".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "30".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "40".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "50".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::ApproxPercentile("value".to_string(), "0.5".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Median of [10,20,30,40,50] is 30
        assert_eq!(res, "30");
    }

    #[test]
    fn test_approx_percentile_large_sampled() {
        // Generate a large dataset; values increasing from 1..=5000
        let schema = vec!["id".to_string(), "value".to_string()];
        let mut rows: Vec<Row> = Vec::new();
        for i in 1..=5000 {
            rows.push(Row::from_values(&schema, vec![i.to_string(), i.to_string()]).unwrap());
        }
        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::ApproxPercentile("value".to_string(), "0.5".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // For 1..=5000 median is ~2500.5; allow a generous tolerance due to sampling
        let val: f64 = res.parse().unwrap();
        assert!((val - 2500.5).abs() < 150.0);
    }

    #[test]
    fn test_var_samp_basic() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "1".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "3".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "5".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::VarSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Sample variance = 2.5
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2.5).abs() < 0.0001);
    }

    #[test]
    fn test_var_samp_single_value_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows =
            vec![Row::from_values(&schema, vec!["1".to_string(), "42".to_string()]).unwrap()];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::VarSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_var_samp_skips_null_values() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "6".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::VarSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Values: 2,4,6 => sample variance = 4
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 4.0).abs() < 0.0001);
    }

    #[test]
    fn test_var_samp_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::VarSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_corr_basic() {
        let schema = vec!["id".to_string(), "x".to_string(), "y".to_string()];
        let rows = vec![
            Row::from_values(
                &schema,
                vec!["1".to_string(), "1".to_string(), "1".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["2".to_string(), "2".to_string(), "2".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["3".to_string(), "3".to_string(), "3".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["4".to_string(), "4".to_string(), "4".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["5".to_string(), "5".to_string(), "5".to_string()],
            )
            .unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Corr("x".to_string(), "y".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_corr_single_value_returns_null() {
        let schema = vec!["id".to_string(), "x".to_string(), "y".to_string()];
        let rows = vec![Row::from_values(
            &schema,
            vec!["1".to_string(), "42".to_string(), "43".to_string()],
        )
        .unwrap()];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Corr("x".to_string(), "y".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_corr_skips_null_values() {
        let schema = vec!["id".to_string(), "x".to_string(), "y".to_string()];
        let rows = vec![
            Row::from_values(
                &schema,
                vec!["1".to_string(), "1".to_string(), "1".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["2".to_string(), "2".to_string(), "NULL".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["3".to_string(), "3".to_string(), "3".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["4".to_string(), "".to_string(), "4".to_string()],
            )
            .unwrap(),
            Row::from_values(
                &schema,
                vec!["5".to_string(), "5".to_string(), "5".to_string()],
            )
            .unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::Corr("x".to_string(), "y".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Remaining pairs: (1,1), (3,3), (5,5) -> correlation 1.0
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_stddev_pop_basic() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "1".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "3".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "5".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevPop("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Population variance = 2, stddev = sqrt(2)
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2f64.sqrt()).abs() < 0.0001);
    }

    #[test]
    fn test_stddev_pop_single_value() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows =
            vec![Row::from_values(&schema, vec!["1".to_string(), "42".to_string()]).unwrap()];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevPop("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "0");
    }

    #[test]
    fn test_stddev_pop_skips_null_values() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "6".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevPop("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Values: 2,4,6; mean = 4; variance = ((2-4)^2 + (4-4)^2 + (6-4)^2) / 3 = (4+0+4)/3 = 2.666...
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2.666666_f64.sqrt()).abs() < 0.001);
    }

    #[test]
    fn test_stddev_pop_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevPop("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_stddev_samp_basic() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "1".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "3".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "5".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Sample variance = 2.5, stddev = sqrt(2.5)
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2.5f64.sqrt()).abs() < 0.0001);
    }

    #[test]
    fn test_stddev_samp_single_value() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows =
            vec![Row::from_values(&schema, vec!["1".to_string(), "42".to_string()]).unwrap()];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }

    #[test]
    fn test_stddev_samp_skips_null_values() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "2".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["3".to_string(), "4".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["4".to_string(), "".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["5".to_string(), "6".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        // Values: 2,4,6 => sample variance = ( (2-4)^2 + (4-4)^2 + (6-4)^2 ) / (3-1) = 8/2 = 4, stddev = 2
        let result_f64: f64 = res.parse().unwrap();
        assert!((result_f64 - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_stddev_samp_all_nulls_returns_null() {
        let schema = vec!["id".to_string(), "value".to_string()];
        let rows = vec![
            Row::from_values(&schema, vec!["1".to_string(), "NULL".to_string()]).unwrap(),
            Row::from_values(&schema, vec!["2".to_string(), "".to_string()]).unwrap(),
        ];

        let row_refs: Vec<&Row> = rows.iter().collect();
        let agg = super::AggregateColumn::StddevSamp("value".to_string());
        let res = super::compute_aggregate(&agg, &row_refs, &schema);
        assert_eq!(res, "NULL");
    }
}
