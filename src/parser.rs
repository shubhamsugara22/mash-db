use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Select,
    Distinct,
    From,
    Where,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Create,
    Drop,
    Alter,
    Add,
    Column,
    Rename,
    To,
    Begin,
    Commit,
    Rollback,
    Transaction,
    Show,
    Truncate,
    Table,
    Order,
    By,
    Asc,
    Desc,
    Limit,
    Offset,
    Group,
    Having,
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Join,
    Inner,
    Left,
    Right,
    On,
    In,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
    Is,
    Not,
    Null,
    Like,
    Between,
    Comma,
    LParen,
    RParen,
    Star,
    Dot, // For table.column syntax
    Identifier(String),
    String(String),
    Number(u32),
}

// JOIN type representation
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

// JOIN clause representation
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: String,
    pub on_left: String,  // left_table.column
    pub on_right: String, // right_table.column
}

// ALTER TABLE action representation
#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableAction {
    Rename(String),
    AddColumn(String),
    DropColumn(String),
}

// Aggregate function representation
#[derive(Debug, Clone)]
pub enum AggregateFunc {
    Count(Option<String>), // COUNT(*) or COUNT(column)
    Sum(String),           // SUM(column)
    Avg(String),           // AVG(column)
    Min(String),           // MIN(column)
    Max(String),           // MAX(column)
}

// Column in SELECT can be a regular column or an aggregate
#[derive(Debug, Clone)]
pub enum SelectColumn {
    Column(String),
    Aggregate(AggregateFunc),
    Star,
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => continue,
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                ident.push(c);
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' || ch == '@' || ch == '.' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let token = match ident.to_uppercase().as_str() {
                    "SELECT" => Token::Select,
                    "DISTINCT" => Token::Distinct,
                    "FROM" => Token::From,
                    "WHERE" => Token::Where,
                    "INSERT" => Token::Insert,
                    "INTO" => Token::Into,
                    "VALUES" => Token::Values,
                    "UPDATE" => Token::Update,
                    "SET" => Token::Set,
                    "DELETE" => Token::Delete,
                    "CREATE" => Token::Create,
                    "DROP" => Token::Drop,
                    "ALTER" => Token::Alter,
                    "ADD" => Token::Add,
                    "COLUMN" => Token::Column,
                    "RENAME" => Token::Rename,
                    "TO" => Token::To,
                    "BEGIN" => Token::Begin,
                    "COMMIT" => Token::Commit,
                    "ROLLBACK" => Token::Rollback,
                    "TRANSACTION" => Token::Transaction,
                    "SHOW" => Token::Show,
                    "TRUNCATE" => Token::Truncate,
                    "TABLE" => Token::Table,
                    "ORDER" => Token::Order,
                    "BY" => Token::By,
                    "ASC" => Token::Asc,
                    "DESC" => Token::Desc,
                    "LIMIT" => Token::Limit,
                    "OFFSET" => Token::Offset,
                    "GROUP" => Token::Group,
                    "HAVING" => Token::Having,
                    "COUNT" => Token::Count,
                    "SUM" => Token::Sum,
                    "AVG" => Token::Avg,
                    "MIN" => Token::Min,
                    "MAX" => Token::Max,
                    "JOIN" => Token::Join,
                    "INNER" => Token::Inner,
                    "LEFT" => Token::Left,
                    "RIGHT" => Token::Right,
                    "ON" => Token::On,
                    "IN" => Token::In,
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "IS" => Token::Is,
                    "NOT" => Token::Not,
                    "NULL" => Token::Null,
                    "LIKE" => Token::Like,
                    "BETWEEN" => Token::Between,
                    _ => Token::Identifier(ident),
                };
                tokens.push(token);
            }
            '0'..='9' => {
                let mut num = String::new();
                num.push(c);
                while let Some(&ch) = chars.peek() {
                    if ch.is_digit(10) {
                        num.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Number(num.parse().unwrap()));
            }
            '"' | '\'' => {
                let mut str = String::new();
                while let Some(ch) = chars.next() {
                    if ch == c {
                        break;
                    }
                    str.push(ch);
                }
                tokens.push(Token::String(str));
            }
            '=' => tokens.push(Token::Eq),
            ',' => tokens.push(Token::Comma),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            '*' => tokens.push(Token::Star),
            '.' => tokens.push(Token::Dot),
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                }
            }
            _ => {} // ignore
        }
    }
    tokens
}

fn token_to_sql(token: &Token) -> String {
    match token {
        Token::Select => "SELECT".to_string(),
        Token::Distinct => "DISTINCT".to_string(),
        Token::From => "FROM".to_string(),
        Token::Where => "WHERE".to_string(),
        Token::Insert => "INSERT".to_string(),
        Token::Into => "INTO".to_string(),
        Token::Values => "VALUES".to_string(),
        Token::Update => "UPDATE".to_string(),
        Token::Set => "SET".to_string(),
        Token::Delete => "DELETE".to_string(),
        Token::Create => "CREATE".to_string(),
        Token::Drop => "DROP".to_string(),
        Token::Alter => "ALTER".to_string(),
        Token::Add => "ADD".to_string(),
        Token::Column => "COLUMN".to_string(),
        Token::Rename => "RENAME".to_string(),
        Token::To => "TO".to_string(),
        Token::Begin => "BEGIN".to_string(),
        Token::Commit => "COMMIT".to_string(),
        Token::Rollback => "ROLLBACK".to_string(),
        Token::Transaction => "TRANSACTION".to_string(),
        Token::Show => "SHOW".to_string(),
        Token::Truncate => "TRUNCATE".to_string(),
        Token::Table => "TABLE".to_string(),
        Token::Order => "ORDER".to_string(),
        Token::By => "BY".to_string(),
        Token::Asc => "ASC".to_string(),
        Token::Desc => "DESC".to_string(),
        Token::Limit => "LIMIT".to_string(),
        Token::Offset => "OFFSET".to_string(),
        Token::Group => "GROUP".to_string(),
        Token::Having => "HAVING".to_string(),
        Token::Count => "COUNT".to_string(),
        Token::Sum => "SUM".to_string(),
        Token::Avg => "AVG".to_string(),
        Token::Min => "MIN".to_string(),
        Token::Max => "MAX".to_string(),
        Token::Join => "JOIN".to_string(),
        Token::Inner => "INNER".to_string(),
        Token::Left => "LEFT".to_string(),
        Token::Right => "RIGHT".to_string(),
        Token::On => "ON".to_string(),
        Token::In => "IN".to_string(),
        Token::Eq => "=".to_string(),
        Token::Ne => "!=".to_string(),
        Token::Gt => ">".to_string(),
        Token::Lt => "<".to_string(),
        Token::Ge => ">=".to_string(),
        Token::Le => "<=".to_string(),
        Token::And => "AND".to_string(),
        Token::Or => "OR".to_string(),
        Token::Is => "IS".to_string(),
        Token::Not => "NOT".to_string(),
        Token::Null => "NULL".to_string(),
        Token::Like => "LIKE".to_string(),
        Token::Between => "BETWEEN".to_string(),
        Token::Comma => ",".to_string(),
        Token::LParen => "(".to_string(),
        Token::RParen => ")".to_string(),
        Token::Star => "*".to_string(),
        Token::Dot => ".".to_string(),
        Token::Identifier(s) => s.clone(),
        Token::String(s) => format!("'{}'", s),
        Token::Number(n) => n.to_string(),
    }
}

fn tokens_to_sql(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(token_to_sql)
        .collect::<Vec<String>>()
        .join(" ")
}

// Helper function to parse SELECT columns (can include aggregates)
pub fn parse_select_columns(
    tokens: &[Token],
    i: &mut usize,
) -> Result<Option<Vec<SelectColumn>>, String> {
    match tokens.get(*i) {
        Some(Token::Star) => {
            *i += 1;
            Ok(Some(vec![SelectColumn::Star]))
        }
        Some(Token::Where) => {
            // SELECT WHERE without column specification means SELECT *
            Ok(None)
        }
        Some(Token::Count)
        | Some(Token::Sum)
        | Some(Token::Avg)
        | Some(Token::Min)
        | Some(Token::Max)
        | Some(Token::Identifier(_)) => {
            let mut cols = Vec::new();

            loop {
                let col = match tokens.get(*i) {
                    Some(Token::Count) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after COUNT".to_string());
                        }
                        *i += 1;

                        // Check for DISTINCT keyword
                        let has_distinct = if tokens.get(*i) == Some(&Token::Distinct) {
                            *i += 1;
                            true
                        } else {
                            false
                        };

                        if tokens.get(*i) == Some(&Token::Star) {
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after COUNT(*)".to_string());
                            }
                            *i += 1;
                            if has_distinct {
                                return Err("COUNT(DISTINCT *) is not supported".to_string());
                            }
                            SelectColumn::Aggregate(AggregateFunc::Count(None))
                        } else if let Some(Token::Identifier(col)) = tokens.get(*i) {
                            let col_name = col.clone();
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after COUNT(col)".to_string());
                            }
                            *i += 1;
                            if has_distinct {
                                // Format as "count(distinct col)" for main.rs to parse
                                SelectColumn::Aggregate(AggregateFunc::Count(Some(format!(
                                    "distinct {}",
                                    col_name
                                ))))
                            } else {
                                SelectColumn::Aggregate(AggregateFunc::Count(Some(col_name)))
                            }
                        } else {
                            return Err("Expected * or column after COUNT(".to_string());
                        }
                    }
                    Some(Token::Sum) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SUM".to_string());
                        }
                        *i += 1;
                        if let Some(Token::Identifier(col)) = tokens.get(*i) {
                            let col_name = col.clone();
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after SUM(col)".to_string());
                            }
                            *i += 1;
                            SelectColumn::Aggregate(AggregateFunc::Sum(col_name))
                        } else {
                            return Err("Expected column after SUM(".to_string());
                        }
                    }
                    Some(Token::Avg) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after AVG".to_string());
                        }
                        *i += 1;
                        if let Some(Token::Identifier(col)) = tokens.get(*i) {
                            let col_name = col.clone();
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after AVG(col)".to_string());
                            }
                            *i += 1;
                            SelectColumn::Aggregate(AggregateFunc::Avg(col_name))
                        } else {
                            return Err("Expected column after AVG(".to_string());
                        }
                    }
                    Some(Token::Min) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after MIN".to_string());
                        }
                        *i += 1;
                        if let Some(Token::Identifier(col)) = tokens.get(*i) {
                            let col_name = col.clone();
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after MIN(col)".to_string());
                            }
                            *i += 1;
                            SelectColumn::Aggregate(AggregateFunc::Min(col_name))
                        } else {
                            return Err("Expected column after MIN(".to_string());
                        }
                    }
                    Some(Token::Max) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after MAX".to_string());
                        }
                        *i += 1;
                        if let Some(Token::Identifier(col)) = tokens.get(*i) {
                            let col_name = col.clone();
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after MAX(col)".to_string());
                            }
                            *i += 1;
                            SelectColumn::Aggregate(AggregateFunc::Max(col_name))
                        } else {
                            return Err("Expected column after MAX(".to_string());
                        }
                    }
                    Some(Token::Identifier(col)) => {
                        let col_name = col.clone();
                        *i += 1;
                        SelectColumn::Column(col_name)
                    }
                    _ => return Err("Expected column or aggregate function".to_string()),
                };

                cols.push(col);

                if tokens.get(*i) == Some(&Token::Comma) {
                    *i += 1;
                } else {
                    break;
                }
            }
            Ok(Some(cols))
        }
        _ => Err("Expected column list, *, or WHERE".to_string()),
    }
}

pub fn parse_select(
    input: &str,
) -> Result<
    (
        bool,                                                 // distinct
        Option<Vec<String>>, // columns (keeping as String for backward compat)
        Option<String>,      // from_table - explicit table name
        Option<JoinClause>,  // join clause
        Option<(Vec<(String, String, String)>, Vec<String>)>, // where clause
        Option<Vec<String>>, // group by columns
        Option<(Vec<(String, String, String)>, Vec<String>)>, // having clause
        Option<(String, bool)>, // (column, is_asc)
        Option<u32>,         // limit
        Option<u32>,         // offset
    ),
    String,
> {
    let tokens = tokenize(input);
    parse_select_tokens(&tokens)
}

fn parse_select_tokens(
    tokens: &[Token],
) -> Result<
    (
        bool,
        Option<Vec<String>>,
        Option<String>,
        Option<JoinClause>,
        Option<(Vec<(String, String, String)>, Vec<String>)>,
        Option<Vec<String>>,
        Option<(Vec<(String, String, String)>, Vec<String>)>,
        Option<(String, bool)>,
        Option<u32>,
        Option<u32>,
    ),
    String,
> {
    let mut alias_map: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Select) {
        return Err("Expected SELECT".to_string());
    }
    i += 1;

    // Check for DISTINCT
    let distinct = if tokens.get(i) == Some(&Token::Distinct) {
        i += 1;
        true
    } else {
        false
    };

    let resolve_alias = |name: &str, alias_map: &HashMap<String, String>| -> String {
        if let Some(idx) = name.find('.') {
            let (prefix, rest) = name.split_at(idx);
            let prefix_l = prefix.to_lowercase();
            let rest = &rest[1..];
            if let Some(real) = alias_map.get(&prefix_l) {
                return format!("{}.{}", real, rest);
            }
        } else {
            let name_l = name.to_lowercase();
            if let Some(real) = alias_map.get(&name_l) {
                return real.clone();
            }
        }
        name.to_string()
    };

    let columns = match tokens.get(i) {
        Some(Token::Star) => {
            i += 1;
            None // means select all
        }
        Some(Token::Where) => {
            // SELECT WHERE without column specification means SELECT *
            None
        }
        Some(Token::Identifier(_))
        | Some(Token::Count)
        | Some(Token::Sum)
        | Some(Token::Avg)
        | Some(Token::Min)
        | Some(Token::Max) => {
            // Use the helper function to parse columns (which might include aggregates)
            match parse_select_columns(&tokens, &mut i) {
                Ok(Some(select_cols)) => {
                    // Convert SelectColumn to String for backward compatibility
                    // For now, just use column names and ignore aggregate function info
                    let cols: Vec<String> = select_cols
                        .iter()
                        .map(|col| match col {
                            SelectColumn::Column(name) => name.clone(),
                            SelectColumn::Star => "*".to_string(),
                            SelectColumn::Aggregate(agg) => match agg {
                                AggregateFunc::Count(Some(name)) => format!("count({})", name),
                                AggregateFunc::Count(None) => "count(*)".to_string(),
                                AggregateFunc::Sum(name) => format!("sum({})", name),
                                AggregateFunc::Avg(name) => format!("avg({})", name),
                                AggregateFunc::Min(name) => format!("min({})", name),
                                AggregateFunc::Max(name) => format!("max({})", name),
                            },
                        })
                        .collect();
                    Some(cols)
                }
                Ok(None) => None,
                Err(e) => return Err(e),
            }
        }
        _ => return Err("Expected column list, *, or WHERE".to_string()),
    };

    // Parse FROM clause (with optional alias)
    let from_table = if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(table_name)) = tokens.get(i) {
            let table = table_name.clone();
            i += 1;
            if let Some(Token::Identifier(alias)) = tokens.get(i) {
                alias_map.insert(alias.to_lowercase(), table.clone());
                i += 1;
            }
            Some(table)
        } else {
            return Err("Expected table name after FROM".to_string());
        }
    } else {
        None
    };

    // Parse JOIN clause
    let join = if matches!(
        tokens.get(i),
        Some(Token::Join) | Some(Token::Inner) | Some(Token::Left) | Some(Token::Right)
    ) {
        let join_type = match tokens.get(i) {
            Some(Token::Join) => {
                i += 1;
                JoinType::Inner // JOIN is treated as INNER JOIN
            }
            Some(Token::Inner) => {
                i += 1;
                if tokens.get(i) != Some(&Token::Join) {
                    return Err("Expected JOIN after INNER".to_string());
                }
                i += 1;
                JoinType::Inner
            }
            Some(Token::Left) => {
                i += 1;
                if tokens.get(i) != Some(&Token::Join) {
                    return Err("Expected JOIN after LEFT".to_string());
                }
                i += 1;
                JoinType::Left
            }
            Some(Token::Right) => {
                i += 1;
                if tokens.get(i) != Some(&Token::Join) {
                    return Err("Expected JOIN after RIGHT".to_string());
                }
                i += 1;
                JoinType::Right
            }
            _ => return Err("Unexpected JOIN token".to_string()),
        };

        // Parse table name
        let join_table = if let Some(Token::Identifier(table)) = tokens.get(i) {
            let t = table.clone();
            i += 1;
            if let Some(Token::Identifier(alias)) = tokens.get(i) {
                alias_map.insert(alias.to_lowercase(), t.clone());
                i += 1;
            }
            t
        } else {
            return Err("Expected table name after JOIN".to_string());
        };

        // Parse ON clause
        if tokens.get(i) != Some(&Token::On) {
            return Err("Expected ON after JOIN table".to_string());
        }
        i += 1;

        // Parse left side (table.column)
        let on_left = if let Some(Token::Identifier(left)) = tokens.get(i) {
            let l = resolve_alias(left, &alias_map);
            i += 1;
            l
        } else {
            return Err("Expected column reference in ON clause".to_string());
        };

        // Expect = operator
        if tokens.get(i) != Some(&Token::Eq) {
            return Err("Expected = in ON clause".to_string());
        }
        i += 1;

        // Parse right side (table.column)
        let on_right = if let Some(Token::Identifier(right)) = tokens.get(i) {
            let r = resolve_alias(right, &alias_map);
            i += 1;
            r
        } else {
            return Err("Expected column reference in ON clause".to_string());
        };

        Some(JoinClause {
            join_type,
            table: join_table,
            on_left,
            on_right,
        })
    } else {
        None
    };

    let where_clause = if tokens.get(i) == Some(&Token::Where) {
        i += 1;
        let mut conditions = Vec::new();
        let mut operators = Vec::new();

        // Parse first condition
        if let Some(Token::Identifier(col)) = tokens.get(i) {
            i += 1;
            // Check for IS NULL / IS NOT NULL
            if tokens.get(i) == Some(&Token::Is) {
                i += 1;
                let is_not = if tokens.get(i) == Some(&Token::Not) {
                    i += 1;
                    true
                } else {
                    false
                };
                if tokens.get(i) != Some(&Token::Null) {
                    return Err("Expected NULL after IS [NOT]".to_string());
                }
                i += 1;
                let norm_col = resolve_alias(col, &alias_map);
                let op = if is_not { "IS NOT NULL" } else { "IS NULL" };
                conditions.push((norm_col, op.to_string(), String::new()));
            } else if tokens.get(i) == Some(&Token::Between) {
                // Handle BETWEEN operator: column BETWEEN value1 AND value2
                i += 1;
                let val1 = if let Some(Token::String(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else if let Some(Token::Number(v)) = tokens.get(i) {
                    i += 1;
                    v.to_string()
                } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else {
                    return Err("Expected value1 in BETWEEN".to_string());
                };

                // Expect AND keyword
                if tokens.get(i) != Some(&Token::And) {
                    return Err("Expected AND after first value in BETWEEN".to_string());
                }
                i += 1;

                let val2 = if let Some(Token::String(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else if let Some(Token::Number(v)) = tokens.get(i) {
                    i += 1;
                    v.to_string()
                } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else {
                    return Err("Expected value2 in BETWEEN".to_string());
                };

                let norm_col = resolve_alias(col, &alias_map);
                // Store as "BETWEEN" operator with format "val1,val2"
                conditions.push((
                    norm_col,
                    "BETWEEN".to_string(),
                    format!("{},{}", val1, val2),
                ));
            } else if tokens.get(i) == Some(&Token::In) {
                // Handle IN operator: column IN (value1, value2, ...) or column IN (SELECT ...)
                i += 1;
                if tokens.get(i) != Some(&Token::LParen) {
                    return Err("Expected ( after IN".to_string());
                }
                i += 1;

                let norm_col = resolve_alias(col, &alias_map);

                if tokens.get(i) == Some(&Token::Select) {
                    let start = i;
                    let mut depth = 1;
                    while i < tokens.len() {
                        match tokens.get(i) {
                            Some(Token::LParen) => depth += 1,
                            Some(Token::RParen) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }

                    if depth != 0 {
                        return Err("Unclosed subquery in IN".to_string());
                    }

                    let sub_tokens = &tokens[start..i];
                    let subquery_sql = tokens_to_sql(sub_tokens);
                    i += 1; // consume closing ')'
                    conditions.push((norm_col, "IN_SUBQUERY".to_string(), subquery_sql));
                } else {
                    let mut values = Vec::new();
                    loop {
                        let val = if let Some(Token::String(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else if let Some(Token::Number(v)) = tokens.get(i) {
                            i += 1;
                            v.to_string()
                        } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else {
                            return Err("Expected value in IN list".to_string());
                        };
                        values.push(val);

                        if tokens.get(i) == Some(&Token::Comma) {
                            i += 1;
                            continue;
                        } else if tokens.get(i) == Some(&Token::RParen) {
                            i += 1;
                            break;
                        } else {
                            return Err("Expected , or ) in IN list".to_string());
                        }
                    }

                    conditions.push((norm_col, "IN".to_string(), values.join(",")));
                }
            } else {
                let op = match tokens.get(i) {
                    Some(Token::Eq) => "=",
                    Some(Token::Ne) => "!=",
                    Some(Token::Gt) => ">",
                    Some(Token::Lt) => "<",
                    Some(Token::Ge) => ">=",
                    Some(Token::Le) => "<=",
                    Some(Token::Like) => "LIKE",
                    _ => return Err("Expected operator".to_string()),
                };
                i += 1;
                let val = if let Some(Token::String(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else if let Some(Token::Number(v)) = tokens.get(i) {
                    i += 1;
                    v.to_string()
                } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else {
                    return Err("Expected value".to_string());
                };
                let norm_col = resolve_alias(col, &alias_map);
                conditions.push((norm_col, op.to_string(), val));
            }
        } else {
            return Err("Expected column".to_string());
        }

        // Parse additional conditions with AND/OR
        while i < tokens.len() {
            if tokens.get(i) == Some(&Token::And) || tokens.get(i) == Some(&Token::Or) {
                let logical_op = if tokens.get(i) == Some(&Token::And) {
                    "AND"
                } else {
                    "OR"
                }
                .to_string();
                operators.push(logical_op);
                i += 1;

                if let Some(Token::Identifier(col)) = tokens.get(i) {
                    i += 1;
                    // Check for IS NULL / IS NOT NULL
                    if tokens.get(i) == Some(&Token::Is) {
                        i += 1;
                        let is_not = if tokens.get(i) == Some(&Token::Not) {
                            i += 1;
                            true
                        } else {
                            false
                        };
                        if tokens.get(i) != Some(&Token::Null) {
                            return Err("Expected NULL after IS [NOT]".to_string());
                        }
                        i += 1;
                        let norm_col = resolve_alias(col, &alias_map);
                        let op = if is_not { "IS NOT NULL" } else { "IS NULL" };
                        conditions.push((norm_col, op.to_string(), String::new()));
                    } else if tokens.get(i) == Some(&Token::Between) {
                        // Handle BETWEEN operator in subsequent conditions
                        i += 1;
                        let val1 = if let Some(Token::String(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else if let Some(Token::Number(v)) = tokens.get(i) {
                            i += 1;
                            v.to_string()
                        } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else {
                            return Err("Expected value1 in BETWEEN".to_string());
                        };

                        if tokens.get(i) != Some(&Token::And) {
                            return Err("Expected AND after first value in BETWEEN".to_string());
                        }
                        i += 1;

                        let val2 = if let Some(Token::String(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else if let Some(Token::Number(v)) = tokens.get(i) {
                            i += 1;
                            v.to_string()
                        } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else {
                            return Err("Expected value2 in BETWEEN".to_string());
                        };

                        let norm_col = resolve_alias(col, &alias_map);
                        conditions.push((
                            norm_col,
                            "BETWEEN".to_string(),
                            format!("{},{}", val1, val2),
                        ));
                    } else if tokens.get(i) == Some(&Token::In) {
                        // Handle IN operator in subsequent conditions
                        i += 1;
                        if tokens.get(i) != Some(&Token::LParen) {
                            return Err("Expected ( after IN".to_string());
                        }
                        i += 1;

                        let norm_col = resolve_alias(col, &alias_map);

                        if tokens.get(i) == Some(&Token::Select) {
                            let start = i;
                            let mut depth = 1;
                            while i < tokens.len() {
                                match tokens.get(i) {
                                    Some(Token::LParen) => depth += 1,
                                    Some(Token::RParen) => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                i += 1;
                            }

                            if depth != 0 {
                                return Err("Unclosed subquery in IN".to_string());
                            }

                            let sub_tokens = &tokens[start..i];
                            let subquery_sql = tokens_to_sql(sub_tokens);
                            i += 1; // consume closing ')'
                            conditions.push((norm_col, "IN_SUBQUERY".to_string(), subquery_sql));
                        } else {
                            let mut values = Vec::new();
                            loop {
                                let val = if let Some(Token::String(v)) = tokens.get(i) {
                                    i += 1;
                                    v.clone()
                                } else if let Some(Token::Number(v)) = tokens.get(i) {
                                    i += 1;
                                    v.to_string()
                                } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                                    i += 1;
                                    v.clone()
                                } else {
                                    return Err("Expected value in IN list".to_string());
                                };
                                values.push(val);

                                if tokens.get(i) == Some(&Token::Comma) {
                                    i += 1;
                                    continue;
                                } else if tokens.get(i) == Some(&Token::RParen) {
                                    i += 1;
                                    break;
                                } else {
                                    return Err("Expected , or ) in IN list".to_string());
                                }
                            }

                            conditions.push((norm_col, "IN".to_string(), values.join(",")));
                        }
                    } else {
                        let op = match tokens.get(i) {
                            Some(Token::Eq) => "=",
                            Some(Token::Ne) => "!=",
                            Some(Token::Gt) => ">",
                            Some(Token::Lt) => "<",
                            Some(Token::Ge) => ">=",
                            Some(Token::Le) => "<=",
                            Some(Token::Like) => "LIKE",
                            _ => return Err("Expected operator".to_string()),
                        };
                        i += 1;
                        let val = if let Some(Token::String(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else if let Some(Token::Number(v)) = tokens.get(i) {
                            i += 1;
                            v.to_string()
                        } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                            i += 1;
                            v.clone()
                        } else {
                            return Err("Expected value".to_string());
                        };
                        let norm_col = resolve_alias(col, &alias_map);
                        conditions.push((norm_col, op.to_string(), val));
                    }
                } else {
                    return Err("Expected column after AND/OR".to_string());
                }
            } else {
                break;
            }
        }

        Some((conditions, operators))
    } else {
        None
    };

    // Parse GROUP BY clause
    let group_by = if tokens.get(i) == Some(&Token::Group) {
        i += 1;
        if tokens.get(i) != Some(&Token::By) {
            return Err("Expected BY after GROUP".to_string());
        }
        i += 1;

        let mut columns = Vec::new();
        if let Some(Token::Identifier(col)) = tokens.get(i) {
            columns.push(resolve_alias(col, &alias_map));
            i += 1;

            // Parse additional columns separated by commas
            while tokens.get(i) == Some(&Token::Comma) {
                i += 1;
                if let Some(Token::Identifier(col)) = tokens.get(i) {
                    columns.push(resolve_alias(col, &alias_map));
                    i += 1;
                } else {
                    return Err("Expected column after comma in GROUP BY".to_string());
                }
            }
            Some(columns)
        } else {
            return Err("Expected column after GROUP BY".to_string());
        }
    } else {
        None
    };

    // Parse HAVING clause (must come after GROUP BY)
    let having = if tokens.get(i) == Some(&Token::Having) {
        i += 1;
        let mut conditions = Vec::new();
        let mut operators = Vec::new();

        // Parse first condition - can be aggregate function or column
        let col = if let Some(Token::Count) = tokens.get(i) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after COUNT".to_string());
            }
            i += 1;
            let col_str = if tokens.get(i) == Some(&Token::Star) {
                i += 1;
                "count(*)".to_string()
            } else if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                format!("count({})", resolve_alias(col_name, &alias_map))
            } else {
                return Err("Expected * or column after COUNT(".to_string());
            };
            if tokens.get(i) != Some(&Token::RParen) {
                return Err("Expected ) after COUNT".to_string());
            }
            i += 1;
            col_str
        } else if let Some(Token::Sum) = tokens.get(i) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after SUM".to_string());
            }
            i += 1;
            if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                if tokens.get(i) != Some(&Token::RParen) {
                    return Err("Expected ) after SUM".to_string());
                }
                i += 1;
                format!("sum({})", resolve_alias(col_name, &alias_map))
            } else {
                return Err("Expected column after SUM(".to_string());
            }
        } else if let Some(Token::Avg) = tokens.get(i) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after AVG".to_string());
            }
            i += 1;
            if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                if tokens.get(i) != Some(&Token::RParen) {
                    return Err("Expected ) after AVG".to_string());
                }
                i += 1;
                format!("avg({})", resolve_alias(col_name, &alias_map))
            } else {
                return Err("Expected column after AVG(".to_string());
            }
        } else if let Some(Token::Min) = tokens.get(i) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after MIN".to_string());
            }
            i += 1;
            if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                if tokens.get(i) != Some(&Token::RParen) {
                    return Err("Expected ) after MIN".to_string());
                }
                i += 1;
                format!("min({})", resolve_alias(col_name, &alias_map))
            } else {
                return Err("Expected column after MIN(".to_string());
            }
        } else if let Some(Token::Max) = tokens.get(i) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after MAX".to_string());
            }
            i += 1;
            if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                if tokens.get(i) != Some(&Token::RParen) {
                    return Err("Expected ) after MAX".to_string());
                }
                i += 1;
                format!("max({})", col_name)
            } else {
                return Err("Expected column after MAX(".to_string());
            }
        } else if let Some(Token::Identifier(col_name)) = tokens.get(i) {
            i += 1;
            col_name.clone()
        } else {
            return Err("Expected column or aggregate function in HAVING".to_string());
        };

        let op = match tokens.get(i) {
            Some(Token::Eq) => "=",
            Some(Token::Ne) => "!=",
            Some(Token::Gt) => ">",
            Some(Token::Lt) => "<",
            Some(Token::Ge) => ">=",
            Some(Token::Le) => "<=",
            _ => return Err("Expected operator in HAVING".to_string()),
        };
        i += 1;
        let val = if let Some(Token::String(v)) = tokens.get(i) {
            i += 1;
            v.clone()
        } else if let Some(Token::Number(v)) = tokens.get(i) {
            i += 1;
            v.to_string()
        } else if let Some(Token::Identifier(v)) = tokens.get(i) {
            i += 1;
            v.clone()
        } else {
            return Err("Expected value in HAVING".to_string());
        };
        conditions.push((col, op.to_string(), val));

        // Parse additional conditions with AND/OR
        while i < tokens.len() {
            if tokens.get(i) == Some(&Token::And) || tokens.get(i) == Some(&Token::Or) {
                let logical_op = if tokens.get(i) == Some(&Token::And) {
                    "AND"
                } else {
                    "OR"
                }
                .to_string();
                operators.push(logical_op);
                i += 1;

                // Parse next condition - can be aggregate or column
                let col = if let Some(Token::Count) = tokens.get(i) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after COUNT".to_string());
                    }
                    i += 1;
                    let col_str = if tokens.get(i) == Some(&Token::Star) {
                        i += 1;
                        "count(*)".to_string()
                    } else if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                        i += 1;
                        format!("count({})", resolve_alias(col_name, &alias_map))
                    } else {
                        return Err("Expected * or column after COUNT(".to_string());
                    };
                    if tokens.get(i) != Some(&Token::RParen) {
                        return Err("Expected ) after COUNT".to_string());
                    }
                    i += 1;
                    col_str
                } else if let Some(Token::Sum) = tokens.get(i) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after SUM".to_string());
                    }
                    i += 1;
                    if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                        i += 1;
                        if tokens.get(i) != Some(&Token::RParen) {
                            return Err("Expected ) after SUM".to_string());
                        }
                        i += 1;
                        format!("sum({})", resolve_alias(col_name, &alias_map))
                    } else {
                        return Err("Expected column after SUM(".to_string());
                    }
                } else if let Some(Token::Avg) = tokens.get(i) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after AVG".to_string());
                    }
                    i += 1;
                    if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                        i += 1;
                        if tokens.get(i) != Some(&Token::RParen) {
                            return Err("Expected ) after AVG".to_string());
                        }
                        i += 1;
                        format!("avg({})", resolve_alias(col_name, &alias_map))
                    } else {
                        return Err("Expected column after AVG(".to_string());
                    }
                } else if let Some(Token::Min) = tokens.get(i) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after MIN".to_string());
                    }
                    i += 1;
                    if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                        i += 1;
                        if tokens.get(i) != Some(&Token::RParen) {
                            return Err("Expected ) after MIN".to_string());
                        }
                        i += 1;
                        format!("min({})", resolve_alias(col_name, &alias_map))
                    } else {
                        return Err("Expected column after MIN(".to_string());
                    }
                } else if let Some(Token::Max) = tokens.get(i) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after MAX".to_string());
                    }
                    i += 1;
                    if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                        i += 1;
                        if tokens.get(i) != Some(&Token::RParen) {
                            return Err("Expected ) after MAX".to_string());
                        }
                        i += 1;
                        format!("max({})", resolve_alias(col_name, &alias_map))
                    } else {
                        return Err("Expected column after MAX(".to_string());
                    }
                } else if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                    i += 1;
                    resolve_alias(col_name, &alias_map)
                } else {
                    return Err("Expected column or aggregate after AND/OR in HAVING".to_string());
                };

                let op = match tokens.get(i) {
                    Some(Token::Eq) => "=",
                    Some(Token::Ne) => "!=",
                    Some(Token::Gt) => ">",
                    Some(Token::Lt) => "<",
                    Some(Token::Ge) => ">=",
                    Some(Token::Le) => "<=",
                    _ => return Err("Expected operator".to_string()),
                };
                i += 1;
                let val = if let Some(Token::String(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else if let Some(Token::Number(v)) = tokens.get(i) {
                    i += 1;
                    v.to_string()
                } else if let Some(Token::Identifier(v)) = tokens.get(i) {
                    i += 1;
                    v.clone()
                } else {
                    return Err("Expected value".to_string());
                };
                conditions.push((col, op.to_string(), val));
            } else {
                break;
            }
        }

        Some((conditions, operators))
    } else {
        None
    };

    // Parse ORDER BY clause (supports both column names and aggregate functions)
    let order_by = if tokens.get(i) == Some(&Token::Order) {
        i += 1;
        if tokens.get(i) != Some(&Token::By) {
            return Err("Expected BY after ORDER".to_string());
        }
        i += 1;

        // Check if ORDER BY is on an aggregate function
        let col = if matches!(
            tokens.get(i),
            Some(Token::Count)
                | Some(Token::Sum)
                | Some(Token::Avg)
                | Some(Token::Min)
                | Some(Token::Max)
        ) {
            // Parse aggregate function: COUNT(...), SUM(...), etc.
            let agg_start = i;
            let func_name = match tokens.get(i) {
                Some(Token::Count) => "count",
                Some(Token::Sum) => "sum",
                Some(Token::Avg) => "avg",
                Some(Token::Min) => "min",
                Some(Token::Max) => "max",
                _ => unreachable!(),
            };
            i += 1;

            // Expect '(' token
            if tokens.get(i) != Some(&Token::LParen) {
                return Err(format!("Expected ( after {}", func_name));
            }
            i += 1;

            // Handle COUNT(*), COUNT(DISTINCT col), or COUNT(col)
            let inner = if func_name == "count" && tokens.get(i) == Some(&Token::Star) {
                i += 1;
                "*".to_string()
            } else if tokens.get(i) == Some(&Token::Distinct) {
                i += 1;
                if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                    i += 1;
                    format!("distinct {}", col_name)
                } else {
                    return Err("Expected column after DISTINCT".to_string());
                }
            } else if let Some(Token::Identifier(col_name)) = tokens.get(i) {
                i += 1;
                col_name.clone()
            } else {
                return Err(format!("Expected column or * in {}", func_name));
            };

            // Expect ')' token
            if tokens.get(i) != Some(&Token::RParen) {
                return Err(format!("Expected ) after {}(...)", func_name));
            }
            i += 1;

            format!("{}({})", func_name, inner)
        } else if let Some(Token::Identifier(col)) = tokens.get(i) {
            i += 1;
            resolve_alias(col, &alias_map)
        } else {
            return Err("Expected column or aggregate function after ORDER BY".to_string());
        };

        let is_asc = if tokens.get(i) == Some(&Token::Asc) {
            i += 1;
            true
        } else if tokens.get(i) == Some(&Token::Desc) {
            i += 1;
            false
        } else {
            true // Default to ASC if not specified
        };
        Some((col, is_asc))
    } else {
        None
    };

    // Parse LIMIT clause
    let limit = if tokens.get(i) == Some(&Token::Limit) {
        i += 1;
        if let Some(Token::Number(n)) = tokens.get(i) {
            i += 1;
            Some(*n)
        } else {
            return Err("Expected number after LIMIT".to_string());
        }
    } else {
        None
    };

    // Parse OFFSET clause
    let offset = if tokens.get(i) == Some(&Token::Offset) {
        i += 1;
        if let Some(Token::Number(n)) = tokens.get(i) {
            i += 1;
            Some(*n)
        } else {
            return Err("Expected number after OFFSET".to_string());
        }
    } else {
        None
    };

    let columns = columns.map(|cols| {
        cols.into_iter()
            .map(|c| {
                let mut out = c.clone();
                for (alias, real) in alias_map.iter() {
                    let pat = format!("{}.", alias);
                    if out.contains(&pat) {
                        out = out.replace(&pat, &format!("{}.", real));
                    }
                }
                out
            })
            .collect()
    });

    Ok((
        distinct,
        columns,
        from_table,
        join,
        where_clause,
        group_by,
        having,
        order_by,
        limit,
        offset,
    ))
}

pub fn parse_insert(input: &str) -> Result<(Option<String>, u32, String, String), String> {
    let tokens = tokenize(input);
    parse_insert_tokens(&tokens)
}

fn parse_insert_tokens(tokens: &[Token]) -> Result<(Option<String>, u32, String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Insert) {
        return Err("Expected INSERT".to_string());
    }
    i += 1;
    // Check if simple format: INSERT id username email
    if let Some(Token::Number(id)) = tokens.get(i) {
        i += 1;
        let username = if let Some(Token::Identifier(u)) = tokens.get(i) {
            u.clone()
        } else {
            return Err("Expected username".to_string());
        };
        i += 1;
        let email = if let Some(Token::Identifier(e)) = tokens.get(i) {
            e.clone()
        } else {
            return Err("Expected email".to_string());
        };
        i += 1;
        if i != tokens.len() {
            return Err("Extra tokens".to_string());
        }
        return Ok((None, *id, username, email));
    }
    // Full format: INSERT [INTO table] VALUES (id, 'username', 'email')
    let mut table_name: Option<String> = None;
    if tokens.get(i) == Some(&Token::Into) {
        i += 1;
        if let Some(Token::Identifier(name)) = tokens.get(i) {
            table_name = Some(name.clone());
            i += 1;
        }
    }
    if tokens.get(i) != Some(&Token::Values) {
        return Err("Expected VALUES".to_string());
    }
    i += 1;
    if tokens.get(i) != Some(&Token::LParen) {
        return Err("Expected (".to_string());
    }
    i += 1;
    let id = if let Some(Token::Number(n)) = tokens.get(i) {
        *n
    } else {
        return Err("Expected id number".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::Comma) {
        return Err("Expected ,".to_string());
    }
    i += 1;
    let username = if let Some(Token::String(s)) = tokens.get(i) {
        s.clone()
    } else {
        return Err("Expected username string".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::Comma) {
        return Err("Expected ,".to_string());
    }
    i += 1;
    let email = if let Some(Token::String(s)) = tokens.get(i) {
        s.clone()
    } else {
        return Err("Expected email string".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::RParen) {
        return Err("Expected )".to_string());
    }
    i += 1;
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((table_name, id, username, email))
}

pub fn parse_update(input: &str) -> Result<(Option<String>, u32, String, String), String> {
    let tokens = tokenize(input);
    parse_update_tokens(&tokens)
}

fn parse_update_tokens(tokens: &[Token]) -> Result<(Option<String>, u32, String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Update) {
        return Err("Expected UPDATE".to_string());
    }
    i += 1;

    let mut table_name: Option<String> = None;
    let first_ident = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        Some(n)
    } else {
        None
    };

    // If we have an identifier followed by SET, that identifier is the table name
    if first_ident.is_some() && tokens.get(i) == Some(&Token::Set) {
        table_name = first_ident;
    } else if tokens.get(i) != Some(&Token::Set) {
        // If no SET immediately after first token, check if first was consumed
        return Err("Expected SET".to_string());
    }

    if tokens.get(i) != Some(&Token::Set) {
        return Err("Expected SET".to_string());
    }
    i += 1;
    let column = if let Some(Token::Identifier(col)) = tokens.get(i) {
        col.clone()
    } else {
        return Err("Expected column".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::Eq) {
        return Err("Expected =".to_string());
    }
    i += 1;
    let value = if let Some(Token::String(s)) = tokens.get(i) {
        s.clone()
    } else {
        return Err("Expected value".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::Where) {
        return Err("Expected WHERE".to_string());
    }
    i += 1;
    if tokens.get(i) != Some(&Token::Identifier("id".to_string())) {
        return Err("Expected id".to_string());
    }
    i += 1;
    if tokens.get(i) != Some(&Token::Eq) {
        return Err("Expected =".to_string());
    }
    i += 1;
    let id = if let Some(Token::Number(n)) = tokens.get(i) {
        *n
    } else {
        return Err("Expected id number".to_string());
    };
    i += 1;
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((table_name, id, column, value))
}

pub fn parse_delete(input: &str) -> Result<(Option<String>, u32), String> {
    let tokens = tokenize(input);
    parse_delete_tokens(&tokens)
}

fn parse_delete_tokens(tokens: &[Token]) -> Result<(Option<String>, u32), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Delete) {
        return Err("Expected DELETE".to_string());
    }
    i += 1;

    let mut table_name: Option<String> = None;
    if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(name)) = tokens.get(i) {
            table_name = Some(name.clone());
            i += 1;
        }
    }

    if tokens.get(i) != Some(&Token::Where) {
        return Err("Expected WHERE".to_string());
    }
    i += 1;
    if tokens.get(i) != Some(&Token::Identifier("id".to_string())) {
        return Err("Expected id".to_string());
    }
    i += 1;
    if tokens.get(i) != Some(&Token::Eq) {
        return Err("Expected =".to_string());
    }
    i += 1;
    let id = if let Some(Token::Number(n)) = tokens.get(i) {
        *n
    } else {
        return Err("Expected id number".to_string());
    };
    i += 1;
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((table_name, id))
}

pub fn parse_delete_where(input: &str) -> Result<(Option<String>, String, String), String> {
    let tokens = tokenize(input);
    parse_delete_where_tokens(&tokens)
}

fn parse_delete_where_tokens(tokens: &[Token]) -> Result<(Option<String>, String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Delete) {
        return Err("Expected DELETE".to_string());
    }
    i += 1;

    let mut table_name: Option<String> = None;
    if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(name)) = tokens.get(i) {
            table_name = Some(name.clone());
            i += 1;
        }
    }
    if tokens.get(i) != Some(&Token::Where) {
        return Err("Expected WHERE".to_string());
    }
    i += 1;
    let column = if let Some(Token::Identifier(col)) = tokens.get(i) {
        col.clone()
    } else {
        return Err("Expected column".to_string());
    };
    i += 1;
    if tokens.get(i) != Some(&Token::Eq) {
        return Err("Expected =".to_string());
    }
    i += 1;

    // Handle both string values and numeric values
    let value = if let Some(Token::String(s)) = tokens.get(i) {
        s.clone()
    } else if let Some(Token::Number(n)) = tokens.get(i) {
        n.to_string()
    } else {
        return Err("Expected value".to_string());
    };
    i += 1;
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((table_name, column, value))
}

// Parse CREATE TABLE statement
// Syntax: CREATE TABLE table_name (column1 type, column2 type, ...)
// For now, simplified: CREATE TABLE table_name (id, username, email)
pub fn parse_create_table(input: &str) -> Result<(String, Vec<String>), String> {
    let tokens = tokenize(input);
    parse_create_table_tokens(&tokens)
}

fn parse_create_table_tokens(tokens: &[Token]) -> Result<(String, Vec<String>), String> {
    if tokens.len() < 5 {
        return Err("CREATE TABLE requires table name and columns".to_string());
    }

    let mut i = 0;
    if tokens.get(i) != Some(&Token::Create) {
        return Err("Expected CREATE".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Table) {
        return Err("Expected TABLE after CREATE".to_string());
    }
    i += 1;

    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        name.clone()
    } else {
        return Err("Expected table name".to_string());
    };
    i += 1;

    if tokens.get(i) != Some(&Token::LParen) {
        return Err("Expected ( after table name".to_string());
    }
    i += 1;

    let mut columns = Vec::new();
    loop {
        if let Some(Token::Identifier(col)) = tokens.get(i) {
            columns.push(col.clone());
            i += 1;

            // Check for comma or closing paren
            if tokens.get(i) == Some(&Token::Comma) {
                i += 1;
                continue;
            } else if tokens.get(i) == Some(&Token::RParen) {
                i += 1;
                break;
            } else {
                return Err("Expected , or ) in column list".to_string());
            }
        } else {
            return Err("Expected column name".to_string());
        }
    }

    if columns.is_empty() {
        return Err("CREATE TABLE requires at least one column".to_string());
    }

    Ok((table_name, columns))
}

// Parse ALTER TABLE statement
// Syntax:
//   ALTER TABLE table_name RENAME TO new_name
//   ALTER TABLE table_name ADD COLUMN column_name
//   ALTER TABLE table_name DROP COLUMN column_name
pub fn parse_alter_table(input: &str) -> Result<(String, AlterTableAction), String> {
    let tokens = tokenize(input);
    parse_alter_table_tokens(&tokens)
}

fn parse_alter_table_tokens(tokens: &[Token]) -> Result<(String, AlterTableAction), String> {
    if tokens.len() < 5 {
        return Err("ALTER TABLE requires an action".to_string());
    }

    let mut i = 0;
    if tokens.get(i) != Some(&Token::Alter) {
        return Err("Expected ALTER".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Table) {
        return Err("Expected TABLE after ALTER".to_string());
    }
    i += 1;

    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        name.clone()
    } else {
        return Err("Expected table name".to_string());
    };
    i += 1;

    match tokens.get(i) {
        Some(Token::Rename) => {
            i += 1;
            if tokens.get(i) != Some(&Token::To) {
                return Err("Expected TO after RENAME".to_string());
            }
            i += 1;
            let new_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
                name.clone()
            } else {
                return Err("Expected new table name".to_string());
            };
            i += 1;
            if i != tokens.len() {
                return Err("Extra tokens".to_string());
            }
            Ok((table_name, AlterTableAction::Rename(new_name)))
        }
        Some(Token::Add) => {
            i += 1;
            if tokens.get(i) != Some(&Token::Column) {
                return Err("Expected COLUMN after ADD".to_string());
            }
            i += 1;
            let col_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
                name.clone()
            } else {
                return Err("Expected column name".to_string());
            };
            i += 1;
            if i != tokens.len() {
                return Err("Extra tokens".to_string());
            }
            Ok((table_name, AlterTableAction::AddColumn(col_name)))
        }
        Some(Token::Drop) => {
            i += 1;
            if tokens.get(i) != Some(&Token::Column) {
                return Err("Expected COLUMN after DROP".to_string());
            }
            i += 1;
            let col_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
                name.clone()
            } else {
                return Err("Expected column name".to_string());
            };
            i += 1;
            if i != tokens.len() {
                return Err("Extra tokens".to_string());
            }
            Ok((table_name, AlterTableAction::DropColumn(col_name)))
        }
        _ => Err("Expected RENAME, ADD, or DROP in ALTER TABLE".to_string()),
    }
}

// Parse DROP TABLE statement
// Syntax: DROP TABLE table_name
pub fn parse_drop_table(input: &str) -> Result<String, String> {
    let tokens = tokenize(input);
    parse_drop_table_tokens(&tokens)
}

fn parse_drop_table_tokens(tokens: &[Token]) -> Result<String, String> {
    if tokens.len() != 3 {
        return Err("DROP TABLE requires table name".to_string());
    }

    let mut i = 0;
    if tokens.get(i) != Some(&Token::Drop) {
        return Err("Expected DROP".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Table) {
        return Err("Expected TABLE after DROP".to_string());
    }
    i += 1;

    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        name.clone()
    } else {
        return Err("Expected table name".to_string());
    };

    Ok(table_name)
}
pub fn parse_truncate_table(input: &str) -> Result<String, String> {
    let tokens = tokenize(input);
    parse_truncate_table_tokens(&tokens)
}

fn parse_truncate_table_tokens(tokens: &[Token]) -> Result<String, String> {
    let mut i = 0;

    if tokens.get(i) != Some(&Token::Truncate) {
        return Err("Expected TRUNCATE".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Table) {
        return Err("Expected TABLE after TRUNCATE".to_string());
    }
    i += 1;

    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        name.clone()
    } else {
        return Err("Expected table name".to_string());
    };

    Ok(table_name)
}
