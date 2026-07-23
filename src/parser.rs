use std::collections::HashMap;
use std::iter::Peekable;
use std::str::Chars;

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
    Union,
    Exists,
    Upper,
    Lower,
    Length,
    Greatest,
    Least,
    StringAgg,
    Case,
    When,
    Then,
    Else,
    End,
    Coalesce,
    Nullif,
    Trim,
    Cast,
    As,
    Concat,
    If,
    Abs,
    Round,
    Substr,
    Replace,
    Lpad,
    Rpad,
    Reverse,
    Repeat,
    Initcap,
    Floor,
    Ceil,
    Mod,
    Power,
    Sqrt,
    Sign,
    Position,
    Instr,
    SubstringIndex,
    Now,
    Date,
    Time,
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    DateAdd,
    DateSub,
    DateDiff,
    DateTrunc,
    Week,
    Quarter,
    With,
    Index,
    Primary,
    Key,
    Unique,
    RowNumber,
    Rank,
    DenseRank,
    FirstValue,
    LastValue,
    Lead,
    Lag,
    Over,
    Partition,
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
    Count(Option<String>),     // COUNT(*) or COUNT(column)
    Sum(String),               // SUM(column)
    Avg(String),               // AVG(column)
    Min(String),               // MIN(column)
    Max(String),               // MAX(column)
    StringAgg(String, String), // STRING_AGG(expr, sep)
}

// Column in SELECT can be a regular column or an aggregate
#[derive(Debug, Clone)]
pub enum SelectColumn {
    Column(String),
    Aggregate(AggregateFunc),
    Star,
}

#[allow(dead_code)]
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn consume_number_literal(first: char, chars: &mut Peekable<Chars<'_>>) -> (String, bool, bool) {
    let mut literal = String::new();
    literal.push(first);

    let has_sign = first == '-' || first == '+';
    let mut has_decimal = false;
    let mut has_exponent = false;

    // Integer part (or full digits for unsigned input)
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            literal.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    // Support signed decimals without leading zero, e.g. -.5
    if (first == '-' || first == '+') && literal.len() == 1 && chars.peek() == Some(&'.') {
        let mut lookahead = chars.clone();
        lookahead.next();
        if lookahead
            .peek()
            .map(|ch| ch.is_ascii_digit())
            .unwrap_or(false)
        {
            has_decimal = true;
            literal.push(chars.next().unwrap()); // '.'
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    literal.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
        }
    } else if chars.peek() == Some(&'.') {
        // Decimal part only if there is at least one digit after the dot
        let mut lookahead = chars.clone();
        lookahead.next();
        if lookahead
            .peek()
            .map(|ch| ch.is_ascii_digit())
            .unwrap_or(false)
        {
            has_decimal = true;
            literal.push(chars.next().unwrap()); // '.'
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    literal.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
        }
    }

    // Exponent part: e/E followed by optional sign and at least one digit
    if matches!(chars.peek(), Some('e') | Some('E')) {
        let mut lookahead = chars.clone();
        let exp_ch = lookahead.next().unwrap();
        let mut valid = false;
        if let Some(next) = lookahead.peek() {
            if next.is_ascii_digit() {
                valid = true;
            } else if *next == '+' || *next == '-' {
                lookahead.next();
                if lookahead
                    .peek()
                    .map(|d| d.is_ascii_digit())
                    .unwrap_or(false)
                {
                    valid = true;
                }
            }
        }

        if valid {
            has_exponent = true;
            literal.push(chars.next().unwrap_or(exp_ch));
            if matches!(chars.peek(), Some('+') | Some('-')) {
                literal.push(chars.next().unwrap());
            }
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    literal.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
        }
    }

    (literal, has_sign, has_decimal || has_exponent)
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
                    "UNION" => Token::Union,
                    "EXISTS" => Token::Exists,
                    "UPPER" => Token::Upper,
                    "LOWER" => Token::Lower,
                    "LENGTH" => Token::Length,
                    "GREATEST" => Token::Greatest,
                    "LEAST" => Token::Least,
                    "STRING_AGG" => Token::StringAgg,
                    "CASE" => Token::Case,
                    "WHEN" => Token::When,
                    "THEN" => Token::Then,
                    "ELSE" => Token::Else,
                    "END" => Token::End,
                    "COALESCE" => Token::Coalesce,
                    "NULLIF" => Token::Nullif,
                    "TRIM" => Token::Trim,
                    "CAST" => Token::Cast,
                    "AS" => Token::As,
                    "CONCAT" => Token::Concat,
                    "IF" => Token::If,
                    "ABS" => Token::Abs,
                    "ROUND" => Token::Round,
                    "SUBSTR" => Token::Substr,
                    "SUBSTRING" => Token::Substr,
                    "REPLACE" => Token::Replace,
                    "LPAD" => Token::Lpad,
                    "RPAD" => Token::Rpad,
                    "REVERSE" => Token::Reverse,
                    "REPEAT" => Token::Repeat,
                    "INITCAP" => Token::Initcap,
                    "FLOOR" => Token::Floor,
                    "CEIL" => Token::Ceil,
                    "MOD" => Token::Mod,
                    "POWER" => Token::Power,
                    "SQRT" => Token::Sqrt,
                    "SIGN" => Token::Sign,
                    "ROW_NUMBER" => Token::RowNumber,
                    "RANK" => Token::Rank,
                    "DENSE_RANK" => Token::DenseRank,
                    "FIRST_VALUE" => Token::FirstValue,
                    "LAST_VALUE" => Token::LastValue,
                    "LEAD" => Token::Lead,
                    "LAG" => Token::Lag,
                    "OVER" => Token::Over,
                    "PARTITION" => Token::Partition,
                    "POSITION" => Token::Position,
                    "INSTR" => Token::Instr,
                    "SUBSTRING_INDEX" => Token::SubstringIndex,
                    "NOW" => Token::Now,
                    "DATE" => Token::Date,
                    "TIME" => Token::Time,
                    "YEAR" => Token::Year,
                    "MONTH" => Token::Month,
                    "DAY" => Token::Day,
                    "HOUR" => Token::Hour,
                    "MINUTE" => Token::Minute,
                    "SECOND" => Token::Second,
                    "DATE_ADD" => Token::DateAdd,
                    "DATE_SUB" => Token::DateSub,
                    "DATEDIFF" => Token::DateDiff,
                    "DATE_TRUNC" => Token::DateTrunc,
                    "WEEK" => Token::Week,
                    "QUARTER" => Token::Quarter,
                    "WITH" => Token::With,
                    "INDEX" => Token::Index,
                    "PRIMARY" => Token::Primary,
                    "KEY" => Token::Key,
                    "UNIQUE" => Token::Unique,
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
                let (num, has_sign, non_integer_form) = consume_number_literal(c, &mut chars);
                if !has_sign && !non_integer_form {
                    tokens.push(Token::Number(num.parse().unwrap()));
                } else {
                    tokens.push(Token::String(num));
                }
            }
            '-' | '+' => {
                let is_numeric = if let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() {
                        true
                    } else if next == '.' {
                        let mut lookahead = chars.clone();
                        lookahead.next();
                        lookahead
                            .peek()
                            .map(|d| d.is_ascii_digit())
                            .unwrap_or(false)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_numeric {
                    let (num, _, _) = consume_number_literal(c, &mut chars);
                    tokens.push(Token::String(num));
                }
            }
            '"' | '\'' => {
                let mut str = String::new();
                for ch in chars.by_ref() {
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
            '.' => {
                // Support unsigned leading-dot numerics such as .5 and .5e2
                let is_numeric = chars.peek().map(|d| d.is_ascii_digit()).unwrap_or(false);
                if is_numeric {
                    let (num, _, _) = consume_number_literal(c, &mut chars);
                    tokens.push(Token::String(num));
                } else {
                    tokens.push(Token::Dot);
                }
            }
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
        Token::Union => "UNION".to_string(),
        Token::Exists => "EXISTS".to_string(),
        Token::Upper => "UPPER".to_string(),
        Token::Lower => "LOWER".to_string(),
        Token::Length => "LENGTH".to_string(),
        Token::Case => "CASE".to_string(),
        Token::When => "WHEN".to_string(),
        Token::Then => "THEN".to_string(),
        Token::Else => "ELSE".to_string(),
        Token::End => "END".to_string(),
        Token::Coalesce => "COALESCE".to_string(),
        Token::Nullif => "NULLIF".to_string(),
        Token::Trim => "TRIM".to_string(),
        Token::Cast => "CAST".to_string(),
        Token::As => "AS".to_string(),
        Token::Concat => "CONCAT".to_string(),
        Token::If => "IF".to_string(),
        Token::Abs => "ABS".to_string(),
        Token::Round => "ROUND".to_string(),
        Token::Substr => "SUBSTR".to_string(),
        Token::Replace => "REPLACE".to_string(),
        Token::Lpad => "LPAD".to_string(),
        Token::Rpad => "RPAD".to_string(),
        Token::Reverse => "REVERSE".to_string(),
        Token::Repeat => "REPEAT".to_string(),
        Token::Initcap => "INITCAP".to_string(),
        Token::Floor => "FLOOR".to_string(),
        Token::Ceil => "CEIL".to_string(),
        Token::Mod => "MOD".to_string(),
        Token::Power => "POWER".to_string(),
        Token::Sqrt => "SQRT".to_string(),
        Token::Sign => "SIGN".to_string(),
        Token::Greatest => "GREATEST".to_string(),
        Token::Least => "LEAST".to_string(),
        Token::StringAgg => "STRING_AGG".to_string(),
        Token::Position => "POSITION".to_string(),
        Token::Instr => "INSTR".to_string(),
        Token::SubstringIndex => "SUBSTRING_INDEX".to_string(),
        Token::Now => "NOW".to_string(),
        Token::Date => "DATE".to_string(),
        Token::Time => "TIME".to_string(),
        Token::Year => "YEAR".to_string(),
        Token::Month => "MONTH".to_string(),
        Token::Day => "DAY".to_string(),
        Token::Hour => "HOUR".to_string(),
        Token::Minute => "MINUTE".to_string(),
        Token::Second => "SECOND".to_string(),
        Token::DateAdd => "DATE_ADD".to_string(),
        Token::DateSub => "DATE_SUB".to_string(),
        Token::DateDiff => "DATEDIFF".to_string(),
        Token::DateTrunc => "DATE_TRUNC".to_string(),
        Token::Week => "WEEK".to_string(),
        Token::Quarter => "QUARTER".to_string(),
        Token::With => "WITH".to_string(),
        Token::Index => "INDEX".to_string(),
        Token::Primary => "PRIMARY".to_string(),
        Token::Key => "KEY".to_string(),
        Token::Unique => "UNIQUE".to_string(),
        Token::RowNumber => "ROW_NUMBER".to_string(),
        Token::Rank => "RANK".to_string(),
        Token::DenseRank => "DENSE_RANK".to_string(),
        Token::FirstValue => "FIRST_VALUE".to_string(),
        Token::LastValue => "LAST_VALUE".to_string(),
        Token::Lead => "LEAD".to_string(),
        Token::Lag => "LAG".to_string(),
        Token::Over => "OVER".to_string(),
        Token::Partition => "PARTITION".to_string(),
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
        | Some(Token::Upper)
        | Some(Token::Lower)
        | Some(Token::Length)
        | Some(Token::Power)
        | Some(Token::Sqrt)
        | Some(Token::Now)
        | Some(Token::Case)
        | Some(Token::StringAgg)
        | Some(Token::Coalesce)
        | Some(Token::Nullif)
        | Some(Token::Trim)
        | Some(Token::Cast)
        | Some(Token::Concat)
        | Some(Token::If)
        | Some(Token::Abs)
        | Some(Token::Round)
        | Some(Token::Substr)
        | Some(Token::Replace)
        | Some(Token::Lpad)
        | Some(Token::Rpad)
        | Some(Token::Left)
        | Some(Token::Right)
        | Some(Token::Reverse)
        | Some(Token::Repeat)
        | Some(Token::Initcap)
        | Some(Token::Floor)
        | Some(Token::Ceil)
        | Some(Token::Mod)
        | Some(Token::Sign)
        | Some(Token::Greatest)
        | Some(Token::Least)
        | Some(Token::RowNumber)
        | Some(Token::Rank)
        | Some(Token::DenseRank)
        | Some(Token::FirstValue)
        | Some(Token::Lead)
        | Some(Token::Lag)
        | Some(Token::Position)
        | Some(Token::Instr)
        | Some(Token::SubstringIndex)
        | Some(Token::Date)
        | Some(Token::Time)
        | Some(Token::Year)
        | Some(Token::Month)
        | Some(Token::Day)
        | Some(Token::Hour)
        | Some(Token::Minute)
        | Some(Token::Second)
        | Some(Token::DateAdd)
        | Some(Token::DateSub)
        | Some(Token::Week)
        | Some(Token::Quarter)
        | Some(Token::DateDiff)
        | Some(Token::DateTrunc)
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
                    Some(Token::Upper) | Some(Token::Lower) | Some(Token::Length) => {
                        let fn_name = match tokens.get(*i) {
                            Some(Token::Upper) => "upper",
                            Some(Token::Lower) => "lower",
                            Some(Token::Length) => "length",
                            _ => unreachable!(),
                        };
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err(format!("Expected ( after {}", fn_name.to_uppercase()));
                        }
                        *i += 1;
                        let inner = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err(format!(
                                "Expected column name inside {}()",
                                fn_name.to_uppercase()
                            ));
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err(format!(
                                "Expected ) after {}(col)",
                                fn_name.to_uppercase()
                            ));
                        }
                        *i += 1;
                        SelectColumn::Column(format!("{}({})", fn_name, inner))
                    }
                    Some(Token::Replace) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after REPLACE".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside REPLACE()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in REPLACE".to_string());
                        }
                        *i += 1;
                        let from_str = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected from-string in REPLACE".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after from-string in REPLACE".to_string());
                        }
                        *i += 1;
                        let to_str = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected to-string in REPLACE".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after REPLACE arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!(
                            "__replace__:{}\x1F{}\x1F{}",
                            col, from_str, to_str
                        ))
                    }
                    Some(Token::Lpad) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after LPAD".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside LPAD()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in LPAD".to_string());
                        }
                        *i += 1;
                        let width = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected width in LPAD".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after width in LPAD".to_string());
                        }
                        *i += 1;
                        let pad = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected pad-string in LPAD".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after LPAD arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__lpad__:{}\x1F{}\x1F{}", col, width, pad))
                    }
                    Some(Token::Rpad) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after RPAD".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside RPAD()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in RPAD".to_string());
                        }
                        *i += 1;
                        let width = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected width in RPAD".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after width in RPAD".to_string());
                        }
                        *i += 1;
                        let pad = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected pad-string in RPAD".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after RPAD arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__rpad__:{}\x1F{}\x1F{}", col, width, pad))
                    }
                    Some(Token::Left) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after LEFT".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside LEFT()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in LEFT".to_string());
                        }
                        *i += 1;
                        let len = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected length in LEFT".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after LEFT arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__left__:{}\x1F{}", col, len))
                    }
                    Some(Token::Right) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after RIGHT".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside RIGHT()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in RIGHT".to_string());
                        }
                        *i += 1;
                        let len = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected length in RIGHT".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after RIGHT arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__right__:{}\x1F{}", col, len))
                    }
                    Some(Token::Reverse) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after REVERSE".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside REVERSE()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after REVERSE argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__reverse__:{}", col))
                    }
                    Some(Token::Repeat) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after REPEAT".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside REPEAT()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in REPEAT".to_string());
                        }
                        *i += 1;
                        let count = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected count in REPEAT".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after REPEAT arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__repeat__:{}\x1F{}", col, count))
                    }
                    Some(Token::Initcap) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after INITCAP".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside INITCAP()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after INITCAP argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__initcap__:{}", col))
                    }
                    Some(Token::Floor) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after FLOOR".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside FLOOR()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after FLOOR argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__floor__:{}", col))
                    }
                    Some(Token::Ceil) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after CEIL".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside CEIL()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after CEIL argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__ceil__:{}", col))
                    }
                    Some(Token::Mod) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after MOD".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside MOD()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in MOD".to_string());
                        }
                        *i += 1;
                        let divisor = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected divisor in MOD".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after MOD arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__mod__:{}\x1F{}", col, divisor))
                    }
                    Some(Token::Power) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after POWER".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside POWER()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in POWER".to_string());
                        }
                        *i += 1;
                        let exponent = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected exponent in POWER".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after POWER arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__power__:{}\x1F{}", col, exponent))
                    }
                    Some(Token::Sqrt) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SQRT".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside SQRT()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after SQRT argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__sqrt__:{}", col))
                    }
                    Some(Token::Sign) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SIGN".to_string());
                        }
                        *i += 1;
                        let arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else if let Some(Token::Number(n)) = tokens.get(*i) {
                            let s = n.to_string();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column, string, or number in SIGN".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after SIGN argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__sign__:{}", arg))
                    }
                    Some(Token::Greatest) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after GREATEST".to_string());
                        }
                        *i += 1;
                        let mut args: Vec<String> = Vec::new();
                        loop {
                            match tokens.get(*i) {
                                Some(Token::Identifier(c)) => {
                                    args.push(c.clone());
                                    *i += 1;
                                }
                                Some(Token::String(s)) => {
                                    args.push(s.clone());
                                    *i += 1;
                                }
                                Some(Token::Number(n)) => {
                                    args.push(n.to_string());
                                    *i += 1;
                                }
                                _ => return Err("Expected argument in GREATEST()".to_string()),
                            }
                            if tokens.get(*i) == Some(&Token::Comma) {
                                *i += 1;
                                continue;
                            }
                            break;
                        }
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after GREATEST(...)".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__greatest__:{}", args.join("\x1F")))
                    }
                    Some(Token::Least) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after LEAST".to_string());
                        }
                        *i += 1;
                        let mut args: Vec<String> = Vec::new();
                        loop {
                            match tokens.get(*i) {
                                Some(Token::Identifier(c)) => {
                                    args.push(c.clone());
                                    *i += 1;
                                }
                                Some(Token::String(s)) => {
                                    args.push(s.clone());
                                    *i += 1;
                                }
                                Some(Token::Number(n)) => {
                                    args.push(n.to_string());
                                    *i += 1;
                                }
                                _ => return Err("Expected argument in LEAST()".to_string()),
                            }
                            if tokens.get(*i) == Some(&Token::Comma) {
                                *i += 1;
                                continue;
                            }
                            break;
                        }
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after LEAST(...)".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__least__:{}", args.join("\x1F")))
                    }
                    Some(Token::RowNumber)
                    | Some(Token::Rank)
                    | Some(Token::DenseRank)
                    | Some(Token::FirstValue)
                    | Some(Token::LastValue)
                    | Some(Token::Lead)
                    | Some(Token::Lag) => {
                        let token_here = tokens.get(*i).cloned();
                        let is_rank = matches!(token_here, Some(Token::Rank));
                        let is_dense = matches!(token_here, Some(Token::DenseRank));
                        let is_first = matches!(token_here, Some(Token::FirstValue));
                        let is_last = matches!(token_here, Some(Token::LastValue));
                        let is_lead = matches!(token_here, Some(Token::Lead));
                        let is_lag = matches!(token_here, Some(Token::Lag));
                        *i += 1; // consume ROW_NUMBER, RANK, DENSE_RANK, LEAD, or LAG
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after window function".to_string());
                        }
                        *i += 1;

                        // For LEAD/LAG, parse column, offset, and default value
                        let mut window_column = String::new();
                        let mut window_offset = String::from("1");
                        let mut window_default = String::from("NULL");

                        if is_first {
                            // Parse column for FIRST_VALUE(column)
                            if let Some(Token::Identifier(col)) = tokens.get(*i) {
                                window_column = col.clone();
                                *i += 1;
                            } else {
                                return Err("Expected column name after FIRST_VALUE(".to_string());
                            }
                            // Do not consume the closing RParen here; let the
                            // unified check after argument parsing handle it.
                        } else if is_last {
                            // Parse column for LAST_VALUE(column)
                            if let Some(Token::Identifier(col)) = tokens.get(*i) {
                                window_column = col.clone();
                                *i += 1;
                            } else {
                                return Err("Expected column name after LAST_VALUE(".to_string());
                            }
                            // Do not consume the closing RParen here; let the
                            // unified check after argument parsing handle it.
                        } else if is_lead || is_lag {
                            // Parse column
                            if let Some(Token::Identifier(col)) = tokens.get(*i) {
                                window_column = col.clone();
                                *i += 1;
                            } else {
                                return Err(format!(
                                    "Expected column name after {}(",
                                    if is_lead { "LEAD" } else { "LAG" }
                                ));
                            }

                            // Optional: parse offset
                            if tokens.get(*i) == Some(&Token::Comma) {
                                *i += 1;
                                match tokens.get(*i) {
                                    Some(Token::Number(n)) => {
                                        window_offset = n.to_string();
                                        *i += 1;
                                    }
                                    Some(Token::String(s)) => {
                                        window_offset = s.clone();
                                        *i += 1;
                                    }
                                    _ => {
                                        return Err(format!(
                                            "Expected number for {} offset",
                                            if is_lead { "LEAD" } else { "LAG" }
                                        ))
                                    }
                                }

                                // Optional: parse default value
                                if tokens.get(*i) == Some(&Token::Comma) {
                                    *i += 1;
                                    match tokens.get(*i) {
                                        Some(Token::String(s)) => {
                                            window_default = s.clone();
                                            *i += 1;
                                        }
                                        Some(Token::Number(n)) => {
                                            window_default = n.to_string();
                                            *i += 1;
                                        }
                                        Some(Token::Null) => {
                                            window_default = String::from("NULL");
                                            *i += 1;
                                        }
                                        _ => {
                                            return Err(format!(
                                                "Expected value for {} default",
                                                if is_lead { "LEAD" } else { "LAG" }
                                            ))
                                        }
                                    }
                                }
                            }
                        }

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after window function arguments".to_string());
                        }
                        *i += 1;

                        // Optional OVER(...) clause
                        let mut partition_cols: Vec<String> = Vec::new();
                        let mut order_specs: Vec<(String, bool)> = Vec::new();
                        if tokens.get(*i) == Some(&Token::Over) {
                            *i += 1;
                            if tokens.get(*i) != Some(&Token::LParen) {
                                return Err("Expected ( after OVER".to_string());
                            }
                            *i += 1;

                            // Optional PARTITION BY
                            if tokens.get(*i) == Some(&Token::Partition) {
                                *i += 1;
                                if tokens.get(*i) != Some(&Token::By) {
                                    return Err("Expected BY after PARTITION".to_string());
                                }
                                *i += 1;
                                loop {
                                    if let Some(Token::Identifier(c)) = tokens.get(*i) {
                                        partition_cols.push(c.clone());
                                        *i += 1;
                                    } else {
                                        return Err(
                                            "Expected column name in PARTITION BY".to_string()
                                        );
                                    }
                                    if tokens.get(*i) == Some(&Token::Comma) {
                                        *i += 1;
                                        continue;
                                    } else {
                                        break;
                                    }
                                }
                            }

                            // Optional ORDER BY
                            if tokens.get(*i) == Some(&Token::Order) {
                                *i += 1;
                                if tokens.get(*i) != Some(&Token::By) {
                                    return Err("Expected BY after ORDER".to_string());
                                }
                                *i += 1;
                                loop {
                                    if let Some(Token::Identifier(c)) = tokens.get(*i) {
                                        let col_name = c.clone();
                                        *i += 1;
                                        let mut asc = true;
                                        if tokens.get(*i) == Some(&Token::Asc) {
                                            asc = true;
                                            *i += 1;
                                        } else if tokens.get(*i) == Some(&Token::Desc) {
                                            asc = false;
                                            *i += 1;
                                        }
                                        order_specs.push((col_name, asc));
                                    } else {
                                        return Err("Expected column name in ORDER BY".to_string());
                                    }
                                    if tokens.get(*i) == Some(&Token::Comma) {
                                        *i += 1;
                                        continue;
                                    } else {
                                        break;
                                    }
                                }
                            }

                            if tokens.get(*i) != Some(&Token::RParen) {
                                return Err("Expected ) after OVER(...)".to_string());
                            }
                            *i += 1;
                        }

                        // Encode as __row_number__, __rank__, __dense_rank__, __lead__, or __lag__:...
                        let partition_part = partition_cols.join(",");
                        let order_part = order_specs
                            .into_iter()
                            .map(|(c, asc)| if asc { c } else { format!("{}:DESC", c) })
                            .collect::<Vec<String>>()
                            .join(",");
                        if is_lag {
                            SelectColumn::Column(format!(
                                "__lag__:{}\x1F{}\x1F{}\x1F{}\x1F{}",
                                window_column,
                                window_offset,
                                window_default,
                                partition_part,
                                order_part
                            ))
                        } else if is_first {
                            SelectColumn::Column(format!(
                                "__first_value__:{}\x1F{}\x1F{}",
                                window_column, partition_part, order_part
                            ))
                        } else if is_last {
                            SelectColumn::Column(format!(
                                "__last_value__:{}\x1F{}\x1F{}",
                                window_column, partition_part, order_part
                            ))
                        } else if is_lead {
                            SelectColumn::Column(format!(
                                "__lead__:{}\x1F{}\x1F{}\x1F{}\x1F{}",
                                window_column,
                                window_offset,
                                window_default,
                                partition_part,
                                order_part
                            ))
                        } else if is_dense {
                            SelectColumn::Column(format!(
                                "__dense_rank__:{}\x1F{}",
                                partition_part, order_part
                            ))
                        } else if is_rank {
                            SelectColumn::Column(format!(
                                "__rank__:{}\x1F{}",
                                partition_part, order_part
                            ))
                        } else {
                            SelectColumn::Column(format!(
                                "__row_number__:{}\x1F{}",
                                partition_part, order_part
                            ))
                        }
                    }
                    Some(Token::Now) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after NOW".to_string());
                        }
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after NOW".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column("__now__".to_string())
                    }
                    Some(Token::Position) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after POSITION".to_string());
                        }
                        *i += 1;

                        // First argument: substring (can be column or string)
                        let substring = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err(
                                "Expected substring (column or string) in POSITION".to_string()
                            );
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after substring in POSITION".to_string());
                        }
                        *i += 1;

                        // Second argument: string (can be column or string)
                        let string = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err(
                                "Expected string (column or string) in POSITION".to_string()
                            );
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after POSITION arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__position__:{}\x1F{}", substring, string))
                    }
                    Some(Token::Instr) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after INSTR".to_string());
                        }
                        *i += 1;

                        // First argument: string (can be column or string)
                        let string = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected string (column or string) in INSTR".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after string in INSTR".to_string());
                        }
                        *i += 1;

                        // Second argument: substring (can be column or string)
                        let substring = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err(
                                "Expected substring (column or string) in INSTR".to_string()
                            );
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after INSTR arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__instr__:{}\x1F{}", string, substring))
                    }
                    Some(Token::SubstringIndex) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SUBSTRING_INDEX".to_string());
                        }
                        *i += 1;

                        // First argument: string (can be column or string)
                        let string = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err(
                                "Expected string (column or string) in SUBSTRING_INDEX".to_string()
                            );
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after string in SUBSTRING_INDEX".to_string());
                        }
                        *i += 1;

                        // Second argument: delimiter (can be column or string)
                        let delimiter = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected delimiter (column or string) in SUBSTRING_INDEX"
                                .to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after delimiter in SUBSTRING_INDEX".to_string());
                        }
                        *i += 1;

                        // Third argument: count (can be column, string, or number)
                        let count = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else if let Some(Token::Number(n)) = tokens.get(*i) {
                            let n = n.to_string();
                            *i += 1;
                            n
                        } else {
                            return Err(
                                "Expected count (column, string, or number) in SUBSTRING_INDEX"
                                    .to_string(),
                            );
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after SUBSTRING_INDEX arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!(
                            "__substring_index__:{}\x1F{}\x1F{}",
                            string, delimiter, count
                        ))
                    }
                    Some(Token::Date) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DATE".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in DATE()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DATE argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__date__:{}", col))
                    }
                    Some(Token::Time) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after TIME".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in TIME()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after TIME argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__time__:{}", col))
                    }
                    Some(Token::Year) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after YEAR".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in YEAR()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after YEAR argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__year__:{}", col))
                    }
                    Some(Token::Month) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after MONTH".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in MONTH()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after MONTH argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__month__:{}", col))
                    }
                    Some(Token::Day) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DAY".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in DAY()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DAY argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__day__:{}", col))
                    }
                    Some(Token::Hour) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after HOUR".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in HOUR()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after HOUR argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__hour__:{}", col))
                    }
                    Some(Token::Minute) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after MINUTE".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in MINUTE()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after MINUTE argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__minute__:{}", col))
                    }
                    Some(Token::Second) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SECOND".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in SECOND()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after SECOND argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__second__:{}", col))
                    }
                    Some(Token::DateAdd) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DATE_ADD".to_string());
                        }
                        *i += 1;
                        // First argument: date column/string
                        let date_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected date column or string in DATE_ADD()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after date argument in DATE_ADD".to_string());
                        }
                        *i += 1;

                        // Second argument: interval (numeric or column)
                        let interval_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else if let Some(Token::Number(n)) = tokens.get(*i) {
                            let n = n.to_string();
                            *i += 1;
                            n
                        } else {
                            return Err("Expected interval in DATE_ADD()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DATE_ADD arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!(
                            "__date_add__:{}\x1F{}",
                            date_arg, interval_arg
                        ))
                    }
                    Some(Token::DateSub) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DATE_SUB".to_string());
                        }
                        *i += 1;
                        // First argument: date column/string
                        let date_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected date column or string in DATE_SUB()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after date argument in DATE_SUB".to_string());
                        }
                        *i += 1;

                        // Second argument: interval (numeric or column)
                        let interval_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else if let Some(Token::Number(n)) = tokens.get(*i) {
                            let n = n.to_string();
                            *i += 1;
                            n
                        } else {
                            return Err("Expected interval in DATE_SUB()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DATE_SUB arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!(
                            "__date_sub__:{}\x1F{}",
                            date_arg, interval_arg
                        ))
                    }
                    Some(Token::Week) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after WEEK".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in WEEK()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after WEEK argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__week__:{}", col))
                    }
                    Some(Token::Quarter) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after QUARTER".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected column or string in QUARTER()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after QUARTER argument".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__quarter__:{}", col))
                    }
                    Some(Token::DateDiff) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DATEDIFF".to_string());
                        }
                        *i += 1;
                        // First argument: date1
                        let date1_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected date column or string in DATEDIFF()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after first date in DATEDIFF".to_string());
                        }
                        *i += 1;

                        // Second argument: date2
                        let date2_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected date column or string in DATEDIFF()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DATEDIFF arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__datediff__:{}\x1F{}", date1_arg, date2_arg))
                    }
                    Some(Token::DateTrunc) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after DATE_TRUNC".to_string());
                        }
                        *i += 1;
                        // First argument: unit (string like 'year', 'month', 'day')
                        let unit_arg = if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else if let Some(Token::Identifier(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err("Expected unit string in DATE_TRUNC()".to_string());
                        };

                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after unit in DATE_TRUNC".to_string());
                        }
                        *i += 1;

                        // Second argument: date column
                        let date_arg = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else if let Some(Token::String(s)) = tokens.get(*i) {
                            let s = s.clone();
                            *i += 1;
                            s
                        } else {
                            return Err(
                                "Expected date column or string in DATE_TRUNC()".to_string()
                            );
                        };

                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after DATE_TRUNC arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__date_trunc__:{}\x1F{}", unit_arg, date_arg))
                    }
                    Some(Token::If) => {
                        *i += 1; // consume IF
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after IF".to_string());
                        }
                        *i += 1;
                        let cond_col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name in IF condition".to_string());
                        };
                        let op_str = match tokens.get(*i) {
                            Some(Token::Eq) => {
                                *i += 1;
                                "="
                            }
                            Some(Token::Ne) => {
                                *i += 1;
                                "!="
                            }
                            Some(Token::Gt) => {
                                *i += 1;
                                ">"
                            }
                            Some(Token::Lt) => {
                                *i += 1;
                                "<"
                            }
                            Some(Token::Ge) => {
                                *i += 1;
                                ">="
                            }
                            Some(Token::Le) => {
                                *i += 1;
                                "<="
                            }
                            _ => {
                                return Err(
                                    "Expected comparison operator in IF condition".to_string()
                                )
                            }
                        };
                        let cmp_val = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => {
                                return Err("Expected comparison value in IF condition".to_string())
                            }
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after IF condition".to_string());
                        }
                        *i += 1;
                        let then_val = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected then-value in IF".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , between IF then and else".to_string());
                        }
                        *i += 1;
                        let else_val = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected else-value in IF".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after IF arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!(
                            "__if__:{}\x1F{}\x1F{}\x1F{}\x1F{}",
                            cond_col, op_str, cmp_val, then_val, else_val
                        ))
                    }
                    Some(Token::Abs) => {
                        *i += 1; // consume ABS
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after ABS".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside ABS()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after ABS(col)".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__abs__:{}", col))
                    }
                    Some(Token::Round) => {
                        *i += 1; // consume ROUND
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after ROUND".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside ROUND()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in ROUND".to_string());
                        }
                        *i += 1;
                        let digits = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected digit count in ROUND".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after ROUND arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__round__:{}\x1F{}", col, digits))
                    }
                    Some(Token::Substr) => {
                        *i += 1; // consume SUBSTR/SUBSTRING
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after SUBSTR".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside SUBSTR()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after column in SUBSTR".to_string());
                        }
                        *i += 1;
                        let start = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected start position in SUBSTR".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after start in SUBSTR".to_string());
                        }
                        *i += 1;
                        let len = match tokens.get(*i) {
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected length in SUBSTR".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after SUBSTR arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__substr__:{}\x1F{}\x1F{}", col, start, len))
                    }
                    Some(Token::StringAgg) => {
                        *i += 1;
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after STRING_AGG".to_string());
                        }
                        *i += 1;
                        // first arg: expression (identifier or string)
                        let first = match tokens.get(*i) {
                            Some(Token::Identifier(c)) => {
                                let s = c.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected column or string in STRING_AGG".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after first argument in STRING_AGG".to_string());
                        }
                        *i += 1;
                        // second arg: separator (string or identifier)
                        let sep = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Identifier(c)) => {
                                let s = c.clone();
                                *i += 1;
                                s
                            }
                            _ => return Err("Expected separator string in STRING_AGG".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after STRING_AGG arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Aggregate(AggregateFunc::StringAgg(first, sep))
                    }
                    Some(Token::Trim) => {
                        *i += 1; // consume TRIM
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after TRIM".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name inside TRIM()".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after TRIM(col)".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__trim__:{}", col))
                    }
                    Some(Token::Cast) => {
                        *i += 1; // consume CAST
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after CAST".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name as first CAST argument".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::As) {
                            return Err("Expected AS in CAST(col AS type)".to_string());
                        }
                        *i += 1;
                        let cast_type = if let Some(Token::Identifier(t)) = tokens.get(*i) {
                            let t = t.clone();
                            *i += 1;
                            t.to_uppercase()
                        } else {
                            return Err("Expected type after AS in CAST".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after CAST(col AS type)".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__cast__:{}\x1F{}", col, cast_type))
                    }
                    Some(Token::Concat) => {
                        *i += 1; // consume CONCAT
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after CONCAT".to_string());
                        }
                        *i += 1;
                        // Parse two arguments (col or string literal)
                        let parse_concat_arg =
                            |tokens: &[Token], i: &mut usize| -> Result<String, String> {
                                match tokens.get(*i) {
                                    Some(Token::Identifier(s)) => {
                                        let s = s.clone();
                                        *i += 1;
                                        Ok(format!("c:{}", s))
                                    }
                                    Some(Token::String(s)) => {
                                        let s = s.clone();
                                        *i += 1;
                                        Ok(format!("s:{}", s))
                                    }
                                    Some(Token::Number(n)) => {
                                        let n = *n;
                                        *i += 1;
                                        Ok(format!("n:{}", n))
                                    }
                                    _ => Err("Expected column name or string literal in CONCAT"
                                        .to_string()),
                                }
                            };
                        let arg1 = parse_concat_arg(tokens, i)?;
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , between CONCAT arguments".to_string());
                        }
                        *i += 1;
                        let arg2 = parse_concat_arg(tokens, i)?;
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after CONCAT arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("__concat__:{}\x1F{}", arg1, arg2))
                    }
                    Some(Token::Coalesce) | Some(Token::Nullif) => {
                        let fn_prefix = match tokens.get(*i) {
                            Some(Token::Coalesce) => "__coalesce__:",
                            Some(Token::Nullif) => "__nullif__:",
                            _ => unreachable!(),
                        };
                        *i += 1; // consume COALESCE/NULLIF
                        if tokens.get(*i) != Some(&Token::LParen) {
                            return Err("Expected ( after function".to_string());
                        }
                        *i += 1;
                        let col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                            let c = c.clone();
                            *i += 1;
                            c
                        } else {
                            return Err("Expected column name as first argument".to_string());
                        };
                        if tokens.get(*i) != Some(&Token::Comma) {
                            return Err("Expected , after first argument".to_string());
                        }
                        *i += 1;
                        let val = match tokens.get(*i) {
                            Some(Token::String(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Number(n)) => {
                                let n = *n;
                                *i += 1;
                                n.to_string()
                            }
                            Some(Token::Identifier(s)) => {
                                let s = s.clone();
                                *i += 1;
                                s
                            }
                            Some(Token::Null) => {
                                *i += 1;
                                "NULL".to_string()
                            }
                            _ => return Err("Expected value as second argument".to_string()),
                        };
                        if tokens.get(*i) != Some(&Token::RParen) {
                            return Err("Expected ) after arguments".to_string());
                        }
                        *i += 1;
                        SelectColumn::Column(format!("{}{}\x1F{}", fn_prefix, col, val))
                    }
                    Some(Token::Case) => {
                        *i += 1; // consume CASE
                        let mut branches: Vec<(String, String, String, String)> = Vec::new();
                        while tokens.get(*i) == Some(&Token::When) {
                            *i += 1; // consume WHEN
                            let cond_col = if let Some(Token::Identifier(c)) = tokens.get(*i) {
                                let c = c.clone();
                                *i += 1;
                                c
                            } else {
                                return Err("Expected column after WHEN".to_string());
                            };
                            let op = match tokens.get(*i) {
                                Some(Token::Eq) => "=",
                                Some(Token::Ne) => "!=",
                                Some(Token::Gt) => ">",
                                Some(Token::Lt) => "<",
                                Some(Token::Ge) => ">=",
                                Some(Token::Le) => "<=",
                                _ => return Err("Expected comparison operator in WHEN".to_string()),
                            };
                            *i += 1;
                            let cond_val = match tokens.get(*i) {
                                Some(Token::String(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Number(n)) => {
                                    let n = *n;
                                    *i += 1;
                                    n.to_string()
                                }
                                Some(Token::Identifier(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Null) => {
                                    *i += 1;
                                    "NULL".to_string()
                                }
                                _ => {
                                    return Err("Expected value after operator in WHEN".to_string())
                                }
                            };
                            if tokens.get(*i) != Some(&Token::Then) {
                                return Err("Expected THEN after WHEN condition".to_string());
                            }
                            *i += 1;
                            let then_val = match tokens.get(*i) {
                                Some(Token::String(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Number(n)) => {
                                    let n = *n;
                                    *i += 1;
                                    n.to_string()
                                }
                                Some(Token::Identifier(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Null) => {
                                    *i += 1;
                                    "NULL".to_string()
                                }
                                _ => return Err("Expected value after THEN".to_string()),
                            };
                            branches.push((cond_col, op.to_string(), cond_val, then_val));
                        }
                        let else_val: Option<String> = if tokens.get(*i) == Some(&Token::Else) {
                            *i += 1;
                            let ev = match tokens.get(*i) {
                                Some(Token::String(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Number(n)) => {
                                    let n = *n;
                                    *i += 1;
                                    n.to_string()
                                }
                                Some(Token::Identifier(s)) => {
                                    let s = s.clone();
                                    *i += 1;
                                    s
                                }
                                Some(Token::Null) => {
                                    *i += 1;
                                    "NULL".to_string()
                                }
                                _ => return Err("Expected value after ELSE".to_string()),
                            };
                            Some(ev)
                        } else {
                            None
                        };
                        if tokens.get(*i) != Some(&Token::End) {
                            return Err("Expected END after CASE expression".to_string());
                        }
                        *i += 1;
                        // Encode as __case__:col\x1Fop\x1Fval\x1Fthen_val[\x1E...][\ x1E__else__\x1Felse_val]
                        let mut encoded = String::from("__case__:");
                        for (idx, (c, o, v, t)) in branches.iter().enumerate() {
                            if idx > 0 {
                                encoded.push('\x1E');
                            }
                            encoded.push_str(c);
                            encoded.push('\x1F');
                            encoded.push_str(o);
                            encoded.push('\x1F');
                            encoded.push_str(v);
                            encoded.push('\x1F');
                            encoded.push_str(t);
                        }
                        if let Some(ev) = &else_val {
                            encoded.push('\x1E');
                            encoded.push_str("__else__\x1F");
                            encoded.push_str(ev);
                        }
                        SelectColumn::Column(encoded)
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

#[allow(clippy::type_complexity)]
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

#[allow(clippy::type_complexity)]
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

    // Debug: print token at column start for failing FIRST_VALUE parse

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
        | Some(Token::Max)
        | Some(Token::Upper)
        | Some(Token::Lower)
        | Some(Token::Length)
        | Some(Token::Case)
        | Some(Token::Power)
        | Some(Token::Sqrt)
        | Some(Token::Now)
        | Some(Token::Position)
        | Some(Token::Coalesce)
        | Some(Token::Nullif)
        | Some(Token::Trim)
        | Some(Token::Cast)
        | Some(Token::Concat)
        | Some(Token::If)
        | Some(Token::Abs)
        | Some(Token::Round)
        | Some(Token::Substr)
        | Some(Token::Replace)
        | Some(Token::Lpad)
        | Some(Token::Rpad)
        | Some(Token::Left)
        | Some(Token::Right)
        | Some(Token::Reverse)
        | Some(Token::Repeat)
        | Some(Token::Initcap)
        | Some(Token::Floor)
        | Some(Token::Ceil)
        | Some(Token::Mod)
        | Some(Token::Sign)
        | Some(Token::Greatest)
        | Some(Token::Least)
        | Some(Token::RowNumber)
        | Some(Token::FirstValue)
        | Some(Token::Rank)
        | Some(Token::DenseRank)
        | Some(Token::Lead)
        | Some(Token::Lag)
        | Some(Token::Instr)
        | Some(Token::SubstringIndex)
        | Some(Token::Date)
        | Some(Token::Time)
        | Some(Token::Year)
        | Some(Token::Month)
        | Some(Token::Day)
        | Some(Token::Hour)
        | Some(Token::Minute)
        | Some(Token::Second)
        | Some(Token::DateAdd)
        | Some(Token::DateSub)
        | Some(Token::Week)
        | Some(Token::Quarter)
        | Some(Token::DateDiff)
        | Some(Token::DateTrunc) => {
            // Use the helper function to parse columns (which might include aggregates)
            match parse_select_columns(tokens, &mut i) {
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
                                AggregateFunc::StringAgg(expr, sep) => {
                                    format!("string_agg({},{})", expr, sep)
                                }
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

    if tokens
        .iter()
        .any(|t| matches!(t, Token::Greatest | Token::Least))
    {
        eprintln!(
            "DEBUG after parse_select_columns: columns={:?}, i={}",
            columns, i
        );
    }

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
        if tokens.get(i) == Some(&Token::Exists) {
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after EXISTS".to_string());
            }
            i += 1;
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
                return Err("Unclosed subquery in EXISTS".to_string());
            }
            let sub_tokens = &tokens[start..i];
            let subquery_sql = tokens_to_sql(sub_tokens);
            i += 1; // consume closing ')'
            conditions.push((
                "__exists__".to_string(),
                "EXISTS_SUBQUERY".to_string(),
                subquery_sql,
            ));
        } else if tokens.get(i) == Some(&Token::Not) && tokens.get(i + 1) == Some(&Token::Exists) {
            i += 2;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err("Expected ( after NOT EXISTS".to_string());
            }
            i += 1;
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
                return Err("Unclosed subquery in NOT EXISTS".to_string());
            }
            let sub_tokens = &tokens[start..i];
            let subquery_sql = tokens_to_sql(sub_tokens);
            i += 1; // consume closing ')'
            conditions.push((
                "__exists__".to_string(),
                "NOT_EXISTS_SUBQUERY".to_string(),
                subquery_sql,
            ));
        } else if matches!(
            tokens.get(i),
            Some(Token::Upper) | Some(Token::Lower) | Some(Token::Length)
        ) {
            let fn_name = match tokens.get(i) {
                Some(Token::Upper) => "upper",
                Some(Token::Lower) => "lower",
                Some(Token::Length) => "length",
                _ => unreachable!(),
            };
            i += 1;
            if tokens.get(i) != Some(&Token::LParen) {
                return Err(format!("Expected ( after {}", fn_name.to_uppercase()));
            }
            i += 1;
            let inner_col = if let Some(Token::Identifier(c)) = tokens.get(i) {
                let c = c.clone();
                i += 1;
                c
            } else {
                return Err(format!(
                    "Expected column name inside {}()",
                    fn_name.to_uppercase()
                ));
            };
            if tokens.get(i) != Some(&Token::RParen) {
                return Err(format!("Expected ) after {}(col)", fn_name.to_uppercase()));
            }
            i += 1;
            let fn_col = format!("{}({})", fn_name, inner_col);
            let op = match tokens.get(i) {
                Some(Token::Eq) => "=",
                Some(Token::Ne) => "!=",
                Some(Token::Gt) => ">",
                Some(Token::Lt) => "<",
                Some(Token::Ge) => ">=",
                Some(Token::Le) => "<=",
                Some(Token::Like) => "LIKE",
                _ => return Err("Expected operator after string function".to_string()),
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
                return Err("Expected value after operator".to_string());
            };
            conditions.push((fn_col, op.to_string(), val));
        } else if let Some(Token::Identifier(col)) = tokens.get(i) {
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
            } else if tokens.get(i) == Some(&Token::Not) && tokens.get(i + 1) == Some(&Token::In) {
                // Handle NOT IN operator
                i += 2;
                if tokens.get(i) != Some(&Token::LParen) {
                    return Err("Expected ( after NOT IN".to_string());
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
                        return Err("Unclosed subquery in NOT IN".to_string());
                    }
                    let sub_tokens = &tokens[start..i];
                    let subquery_sql = tokens_to_sql(sub_tokens);
                    i += 1;
                    conditions.push((norm_col, "NOT_IN_SUBQUERY".to_string(), subquery_sql));
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
                            return Err("Expected value in NOT IN list".to_string());
                        };
                        values.push(val);
                        if tokens.get(i) == Some(&Token::Comma) {
                            i += 1;
                            continue;
                        } else if tokens.get(i) == Some(&Token::RParen) {
                            i += 1;
                            break;
                        } else {
                            return Err("Expected , or ) in NOT IN list".to_string());
                        }
                    }
                    conditions.push((norm_col, "NOT_IN".to_string(), values.join(",")));
                }
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

                if tokens.get(i) == Some(&Token::Exists) {
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after EXISTS".to_string());
                    }
                    i += 1;
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
                        return Err("Unclosed subquery in EXISTS".to_string());
                    }
                    let sub_tokens = &tokens[start..i];
                    let subquery_sql = tokens_to_sql(sub_tokens);
                    i += 1; // consume closing ')'
                    conditions.push((
                        "__exists__".to_string(),
                        "EXISTS_SUBQUERY".to_string(),
                        subquery_sql,
                    ));
                } else if tokens.get(i) == Some(&Token::Not)
                    && tokens.get(i + 1) == Some(&Token::Exists)
                {
                    i += 2;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err("Expected ( after NOT EXISTS".to_string());
                    }
                    i += 1;
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
                        return Err("Unclosed subquery in NOT EXISTS".to_string());
                    }
                    let sub_tokens = &tokens[start..i];
                    let subquery_sql = tokens_to_sql(sub_tokens);
                    i += 1; // consume closing ')'
                    conditions.push((
                        "__exists__".to_string(),
                        "NOT_EXISTS_SUBQUERY".to_string(),
                        subquery_sql,
                    ));
                } else if matches!(
                    tokens.get(i),
                    Some(Token::Upper) | Some(Token::Lower) | Some(Token::Length)
                ) {
                    let fn_name = match tokens.get(i) {
                        Some(Token::Upper) => "upper",
                        Some(Token::Lower) => "lower",
                        Some(Token::Length) => "length",
                        _ => unreachable!(),
                    };
                    i += 1;
                    if tokens.get(i) != Some(&Token::LParen) {
                        return Err(format!("Expected ( after {}", fn_name.to_uppercase()));
                    }
                    i += 1;
                    let inner_col = if let Some(Token::Identifier(c)) = tokens.get(i) {
                        let c = c.clone();
                        i += 1;
                        c
                    } else {
                        return Err(format!(
                            "Expected column name inside {}()",
                            fn_name.to_uppercase()
                        ));
                    };
                    if tokens.get(i) != Some(&Token::RParen) {
                        return Err(format!("Expected ) after {}(col)", fn_name.to_uppercase()));
                    }
                    i += 1;
                    let fn_col = format!("{}({})", fn_name, inner_col);
                    let op = match tokens.get(i) {
                        Some(Token::Eq) => "=",
                        Some(Token::Ne) => "!=",
                        Some(Token::Gt) => ">",
                        Some(Token::Lt) => "<",
                        Some(Token::Ge) => ">=",
                        Some(Token::Le) => "<=",
                        Some(Token::Like) => "LIKE",
                        _ => return Err("Expected operator after string function".to_string()),
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
                        return Err("Expected value after operator".to_string());
                    };
                    conditions.push((fn_col, op.to_string(), val));
                } else if let Some(Token::Identifier(col)) = tokens.get(i) {
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
                    } else if tokens.get(i) == Some(&Token::Not)
                        && tokens.get(i + 1) == Some(&Token::In)
                    {
                        // Handle NOT IN operator in subsequent conditions
                        i += 2;
                        if tokens.get(i) != Some(&Token::LParen) {
                            return Err("Expected ( after NOT IN".to_string());
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
                                return Err("Unclosed subquery in NOT IN".to_string());
                            }
                            let sub_tokens = &tokens[start..i];
                            let subquery_sql = tokens_to_sql(sub_tokens);
                            i += 1;
                            conditions.push((
                                norm_col,
                                "NOT_IN_SUBQUERY".to_string(),
                                subquery_sql,
                            ));
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
                                    return Err("Expected value in NOT IN list".to_string());
                                };
                                values.push(val);
                                if tokens.get(i) == Some(&Token::Comma) {
                                    i += 1;
                                    continue;
                                } else if tokens.get(i) == Some(&Token::RParen) {
                                    i += 1;
                                    break;
                                } else {
                                    return Err("Expected , or ) in NOT IN list".to_string());
                                }
                            }
                            conditions.push((norm_col, "NOT_IN".to_string(), values.join(",")));
                        }
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
            let _agg_start = i;
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

pub fn parse_insert(input: &str) -> Result<(Option<String>, Vec<String>), String> {
    let tokens = tokenize(input);
    parse_insert_tokens(&tokens)
}

fn parse_insert_tokens(tokens: &[Token]) -> Result<(Option<String>, Vec<String>), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Insert) {
        return Err("Expected INSERT".to_string());
    }
    i += 1;
    // Check if simple format: INSERT id username email
    if let Some(Token::Number(id)) = tokens.get(i) {
        let mut values: Vec<String> = Vec::new();
        values.push(id.to_string());
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
        values.push(username);
        values.push(email);
        return Ok((None, values));
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
    let mut values: Vec<String> = Vec::new();
    loop {
        let token = tokens.get(i).ok_or("Expected value".to_string())?;
        match token {
            Token::Number(n) => values.push(n.to_string()),
            Token::String(s) => values.push(s.clone()),
            Token::Identifier(s) => values.push(s.clone()),
            _ => return Err("Expected value".to_string()),
        }
        i += 1;
        match tokens.get(i) {
            Some(Token::Comma) => {
                i += 1;
                continue;
            }
            Some(Token::RParen) => {
                i += 1;
                break;
            }
            _ => return Err("Expected , or )".to_string()),
        }
    }
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((table_name, values))
}

pub fn parse_update(input: &str) -> Result<(Option<String>, u32, Vec<(String, String)>), String> {
    let tokens = tokenize(input);
    parse_update_tokens(&tokens)
}

fn parse_update_tokens(
    tokens: &[Token],
) -> Result<(Option<String>, u32, Vec<(String, String)>), String> {
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
        return Err("Expected SET".to_string());
    }

    if tokens.get(i) != Some(&Token::Set) {
        return Err("Expected SET".to_string());
    }
    i += 1;

    // Parse one or more col = val assignments separated by commas
    let mut assignments: Vec<(String, String)> = Vec::new();
    loop {
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
        } else if let Some(Token::Number(n)) = tokens.get(i) {
            n.to_string()
        } else if let Some(Token::Identifier(s)) = tokens.get(i) {
            s.clone()
        } else {
            return Err("Expected value".to_string());
        };
        i += 1;
        assignments.push((column, value));
        if tokens.get(i) == Some(&Token::Comma) {
            i += 1; // consume comma, parse next assignment
        } else {
            break; // no more assignments
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
    Ok((table_name, id, assignments))
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
// Syntax: CREATE TABLE table_name (column1 PRIMARY KEY, column2 UNIQUE, column3)
pub fn parse_create_table(
    input: &str,
) -> Result<(String, Vec<String>, Option<String>, Vec<String>), String> {
    let tokens = tokenize(input);
    parse_create_table_tokens(&tokens)
}

fn parse_create_table_tokens(
    tokens: &[Token],
) -> Result<(String, Vec<String>, Option<String>, Vec<String>), String> {
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
    let mut primary_key: Option<String> = None;
    let mut unique_columns: Vec<String> = Vec::new();

    loop {
        if let Some(Token::Identifier(col)) = tokens.get(i) {
            let column_name = col.clone();
            columns.push(column_name.clone());
            i += 1;

            // Skip data type if present (INTEGER, TEXT, etc.)
            if let Some(Token::Identifier(_)) = tokens.get(i) {
                i += 1; // skip data type
            }

            // Check for PRIMARY KEY constraint
            if tokens.get(i) == Some(&Token::Primary) {
                i += 1;
                if tokens.get(i) == Some(&Token::Key) {
                    i += 1;
                    if primary_key.is_some() {
                        return Err("Cannot have multiple PRIMARY KEY columns".to_string());
                    }
                    primary_key = Some(column_name.clone());
                } else {
                    return Err("Expected KEY after PRIMARY".to_string());
                }
            }

            // Check for UNIQUE constraint
            if tokens.get(i) == Some(&Token::Unique) {
                i += 1;
                unique_columns.push(column_name);
            }

            // Check for comma or closing paren
            if tokens.get(i) == Some(&Token::Comma) {
                i += 1;
                continue;
            } else if tokens.get(i) == Some(&Token::RParen) {
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

    Ok((table_name, columns, primary_key, unique_columns))
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

/// Parse `INSERT INTO <table> SELECT ...`
/// Returns (target_table_name, select_sql)
pub fn parse_insert_select(input: &str) -> Result<(String, String), String> {
    let tokens = tokenize(input);
    let mut i = 0;

    if tokens.get(i) != Some(&Token::Insert) {
        return Err("Expected INSERT".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Into) {
        return Err("Expected INTO after INSERT".to_string());
    }
    i += 1;

    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let name = name.clone();
        i += 1;
        name
    } else {
        return Err("Expected table name after INSERT INTO".to_string());
    };

    if tokens.get(i) != Some(&Token::Select) {
        return Err("Expected SELECT after table name".to_string());
    }

    let select_sql = tokens_to_sql(&tokens[i..]);
    Ok((table_name, select_sql))
}

/// CTE (Common Table Expression) representation
#[derive(Debug, Clone)]
pub struct CommonTableExpression {
    pub name: String,
    pub query: String,
}

/// Parse a WITH clause: WITH cte_name AS (SELECT ...) SELECT ...
pub fn parse_cte(input: &str) -> Result<(Option<CommonTableExpression>, String), String> {
    let trimmed = input.trim();
    let upper = trimmed.to_uppercase();

    if !upper.starts_with("WITH ") {
        // No CTE, return None for CTE and original query
        return Ok((None, input.to_string()));
    }

    let tokens = tokenize(trimmed);
    let mut i = 0;

    // Expect: WITH cte_name AS (SELECT ...) SELECT ...
    if tokens.get(i) != Some(&Token::With) {
        return Err("Expected WITH".to_string());
    }
    i += 1;

    // Get CTE name
    let cte_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        n
    } else {
        return Err("Expected CTE name after WITH".to_string());
    };

    // Expect AS
    if tokens.get(i) != Some(&Token::As) {
        return Err("Expected AS after CTE name".to_string());
    }
    i += 1;

    // Expect (
    if tokens.get(i) != Some(&Token::LParen) {
        return Err("Expected ( after AS".to_string());
    }
    i += 1;

    // Find matching closing parenthesis for the CTE query
    let mut paren_depth = 1;
    let cte_start = i;
    while i < tokens.len() && paren_depth > 0 {
        match tokens.get(i) {
            Some(Token::LParen) => paren_depth += 1,
            Some(Token::RParen) => paren_depth -= 1,
            _ => {}
        }
        if paren_depth > 0 {
            i += 1;
        }
    }

    if paren_depth != 0 {
        return Err("Unclosed parenthesis in CTE definition".to_string());
    }

    // Extract CTE query SQL
    let cte_query_tokens = &tokens[cte_start..i];
    let cte_query = tokens_to_sql(cte_query_tokens);
    i += 1; // Skip closing paren

    // Rest is the main SELECT query
    let main_query_tokens = &tokens[i..];
    let main_query = tokens_to_sql(main_query_tokens);

    if main_query.trim().is_empty() {
        return Err("Expected SELECT after CTE definition".to_string());
    }

    let cte = CommonTableExpression {
        name: cte_name,
        query: cte_query,
    };

    Ok((Some(cte), main_query))
}

/// Index specification for CREATE INDEX
#[derive(Debug, Clone)]
pub struct IndexDefinition {
    pub index_name: String,
    pub table_name: String,
    pub column_name: String,
}

/// Parse CREATE INDEX statement
/// Syntax: CREATE INDEX index_name ON table_name (column_name)
pub fn parse_create_index(input: &str) -> Result<IndexDefinition, String> {
    let tokens = tokenize(input);
    let mut i = 0;

    // Expect: CREATE INDEX index_name ON table_name (column_name)
    if tokens.get(i) != Some(&Token::Create) {
        return Err("Expected CREATE".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Index) {
        return Err("Expected INDEX after CREATE".to_string());
    }
    i += 1;

    // Get index name
    let index_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        n
    } else {
        return Err("Expected index name after CREATE INDEX".to_string());
    };

    // Expect ON
    if tokens.get(i) != Some(&Token::On) {
        return Err("Expected ON after index name".to_string());
    }
    i += 1;

    // Get table name
    let table_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        n
    } else {
        return Err("Expected table name after ON".to_string());
    };

    // Expect (
    if tokens.get(i) != Some(&Token::LParen) {
        return Err("Expected ( after table name".to_string());
    }
    i += 1;

    // Get column name
    let column_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        n
    } else {
        return Err("Expected column name inside parentheses".to_string());
    };

    // Expect )
    if tokens.get(i) != Some(&Token::RParen) {
        return Err("Expected ) after column name".to_string());
    }
    i += 1;

    // Should be end of statement
    if i < tokens.len() {
        return Err("Unexpected tokens after CREATE INDEX statement".to_string());
    }

    Ok(IndexDefinition {
        index_name,
        table_name,
        column_name,
    })
}

/// Parse DROP INDEX statement
/// Syntax: DROP INDEX index_name
pub fn parse_drop_index(input: &str) -> Result<String, String> {
    let tokens = tokenize(input);
    let mut i = 0;

    if tokens.get(i) != Some(&Token::Drop) {
        return Err("Expected DROP".to_string());
    }
    i += 1;

    if tokens.get(i) != Some(&Token::Index) {
        return Err("Expected INDEX after DROP".to_string());
    }
    i += 1;

    let index_name = if let Some(Token::Identifier(name)) = tokens.get(i) {
        let n = name.clone();
        i += 1;
        n
    } else {
        return Err("Expected index name after DROP INDEX".to_string());
    };

    // Should be end of statement
    if i < tokens.len() {
        return Err("Unexpected tokens after DROP INDEX statement".to_string());
    }

    Ok(index_name)
}
