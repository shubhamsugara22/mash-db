#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    KeywordSelect,
    Identifier(String),
    Star,
    Comma,
    Semicolon,
    EOF,
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        match c {
            '*' => {
                tokens.push(Token::Star);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            ';' => {
                tokens.push(Token::Semicolon);
                chars.next();
            }
            _ => {
                if is_ident_char(c) {
                    let mut ident = String::new();
                    while let Some(&ch) = chars.peek() {
                        if is_ident_char(ch) {
                            ident.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if ident.eq_ignore_ascii_case("select") {
                        tokens.push(Token::KeywordSelect);
                    } else {
                        tokens.push(Token::Identifier(ident));
                    }
                } else {
                    // Unknown char: skip it (keeps tokenizer forgiving)
                    chars.next();
                }
            }
        }
    }

    tokens.push(Token::EOF);
    tokens
}

pub fn parse_select(
    input: &str,
) -> Result<(Option<Vec<String>>, Option<(String, String)>), String> {
    let input = input.trim();

    if !input.to_lowercase().starts_with("select") {
        return Err("Not a select statement".to_string());
    }

    let rest = input[6..].trim(); // after "select"

    if rest.is_empty() {
        return Ok((None, None));
    }

    let parts: Vec<&str> = rest.splitn(2, " where ").collect();

    if parts.len() == 1 && parts[0].starts_with("where ") {
        let cond_str = &parts[0][6..]; // skip "where "
        let eq_pos = cond_str.find('=').ok_or("Invalid WHERE clause")?;
        let column = cond_str[..eq_pos].trim().to_string();
        let value = cond_str[eq_pos + 1..].trim().to_string();
        return Ok((None, Some((column, value))));
    }

    let columns = if parts[0].trim().is_empty() {
        None
    } else {
        Some(parts[0].split(',').map(|c| c.trim().to_string()).collect())
    };

    let condition = if parts.len() == 2 {
        let cond = parts[1];
        let eq_pos = cond.find('=').ok_or("Invalid WHERE clause")?;
        let column = cond[..eq_pos].trim().to_string();
        let value = cond[eq_pos + 1..].trim().to_string();
        Some((column, value))
    } else {
        None
    };

    Ok((columns, condition))
}

fn parse_select_tokens(tokens: &[Token]) -> Result<Option<Vec<String>>, String> {
    let mut idx = 0;
    // expect KeywordSelect
    match tokens.get(idx) {
        Some(Token::KeywordSelect) => idx += 1,
        _ => return Err("Not a SELECT statement".to_string()),
    }

    // skip any whitespace tokens (tokenizer already removed whitespace)

    // If next is Star or EOF or Semicolon -> select all
    match tokens.get(idx) {
        Some(Token::Star) => return Ok(None),
        Some(Token::EOF) | Some(Token::Semicolon) => return Ok(None),
        _ => {}
    }

    let mut cols = Vec::new();

    loop {
        match tokens.get(idx) {
            Some(Token::Identifier(name)) => {
                cols.push(name.clone());
                idx += 1;
            }
            _ => return Err("Expected column name in SELECT".to_string()),
        }

        match tokens.get(idx) {
            Some(Token::Comma) => {
                idx += 1; // consume comma and continue
            }
            Some(Token::Semicolon) | Some(Token::EOF) => break,
            Some(Token::Identifier(_)) => {
                // allow identifiers without commas if user left spaces
                continue;
            }
            _ => break,
        }
    }

    if cols.is_empty() {
        Ok(None)
    } else {
        Ok(Some(cols))
    }
}
