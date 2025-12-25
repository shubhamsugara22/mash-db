#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Eq,
    Comma,
    LParen,
    RParen,
    Star,
    Identifier(String),
    String(String),
    Number(u32),
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
                    if ch.is_alphanumeric() || ch == '_' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let token = match ident.to_uppercase().as_str() {
                    "SELECT" => Token::Select,
                    "FROM" => Token::From,
                    "WHERE" => Token::Where,
                    "INSERT" => Token::Insert,
                    "INTO" => Token::Into,
                    "VALUES" => Token::Values,
                    "UPDATE" => Token::Update,
                    "SET" => Token::Set,
                    "DELETE" => Token::Delete,
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
            _ => {} // ignore
        }
    }
    tokens
}

pub fn parse_select(
    input: &str,
) -> Result<(Option<Vec<String>>, Option<(String, String)>), String> {
    let tokens = tokenize(input);
    parse_select_tokens(&tokens)
}

fn parse_select_tokens(
    tokens: &[Token],
) -> Result<(Option<Vec<String>>, Option<(String, String)>), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Select) {
        return Err("Expected SELECT".to_string());
    }
    i += 1;
    let columns = if tokens.get(i) == Some(&Token::Star) {
        i += 1;
        None
    } else {
        None
    };
    if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(_)) = tokens.get(i) {
            i += 1;
        }
    }
    let where_clause = if tokens.get(i) == Some(&Token::Where) {
        i += 1;
        if let Some(Token::Identifier(col)) = tokens.get(i) {
            i += 1;
            if tokens.get(i) == Some(&Token::Eq) {
                i += 1;
                if let Some(Token::String(val)) = tokens.get(i) {
                    i += 1;
                    Some((col.clone(), val.clone()))
                } else if let Some(Token::Number(val)) = tokens.get(i) {
                    i += 1;
                    Some((col.clone(), val.to_string()))
                } else {
                    return Err("Expected value".to_string());
                }
            } else {
                return Err("Expected =".to_string());
            }
        } else {
            return Err("Expected column".to_string());
        }
    } else {
        None
    };
    Ok((columns, where_clause))
}

pub fn parse_insert(input: &str) -> Result<(u32, String, String), String> {
    let tokens = tokenize(input);
    parse_insert_tokens(&tokens)
}

fn parse_insert_tokens(tokens: &[Token]) -> Result<(u32, String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Insert) {
        return Err("Expected INSERT".to_string());
    }
    i += 1;
    if tokens.get(i) == Some(&Token::Into) {
        i += 1;
        if let Some(Token::Identifier(_)) = tokens.get(i) {
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
    Ok((id, username, email))
}

pub fn parse_update(input: &str) -> Result<(u32, String, String), String> {
    let tokens = tokenize(input);
    parse_update_tokens(&tokens)
}

fn parse_update_tokens(tokens: &[Token]) -> Result<(u32, String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Update) {
        return Err("Expected UPDATE".to_string());
    }
    i += 1;
    if let Some(Token::Identifier(_)) = tokens.get(i) {
        i += 1;
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
    Ok((id, column, value))
}

pub fn parse_delete(input: &str) -> Result<u32, String> {
    let tokens = tokenize(input);
    parse_delete_tokens(&tokens)
}

fn parse_delete_tokens(tokens: &[Token]) -> Result<u32, String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Delete) {
        return Err("Expected DELETE".to_string());
    }
    i += 1;
    if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(_)) = tokens.get(i) {
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
    Ok(id)
}

pub fn parse_delete_where(input: &str) -> Result<(String, String), String> {
    let tokens = tokenize(input);
    parse_delete_where_tokens(&tokens)
}

fn parse_delete_where_tokens(tokens: &[Token]) -> Result<(String, String), String> {
    let mut i = 0;
    if tokens.get(i) != Some(&Token::Delete) {
        return Err("Expected DELETE".to_string());
    }
    i += 1;
    if tokens.get(i) == Some(&Token::From) {
        i += 1;
        if let Some(Token::Identifier(_)) = tokens.get(i) {
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
    let value = if let Some(Token::String(s)) = tokens.get(i) {
        s.clone()
    } else {
        return Err("Expected value".to_string());
    };
    i += 1;
    if i != tokens.len() {
        return Err("Extra tokens".to_string());
    }
    Ok((column, value))
}
