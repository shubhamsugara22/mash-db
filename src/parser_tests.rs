#[cfg(test)]
mod tests {
    use crate::parser::*;

    #[test]
    fn test_tokenize_simple_select() {
        let tokens = tokenize("SELECT * FROM users");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0], Token::Select);
        assert_eq!(tokens[1], Token::Star);
        assert_eq!(tokens[2], Token::From);
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = tokenize("id > 5 AND id <= 10");
        assert!(tokens.contains(&Token::Gt));
        assert!(tokens.contains(&Token::And));
        assert!(tokens.contains(&Token::Le));
    }

    #[test]
    fn test_parse_insert_simple_format() {
        let result = parse_insert("INSERT 1 alice alice@example.com");
        assert!(result.is_ok());
        let (id, username, email) = result.unwrap();
        assert_eq!(id, 1);
        assert_eq!(username, "alice");
        assert_eq!(email, "alice@example.com");
    }

    #[test]
    fn test_parse_insert_full_format() {
        let result = parse_insert("INSERT INTO users VALUES (1, 'alice', 'alice@example.com')");
        assert!(result.is_ok());
        let (id, username, email) = result.unwrap();
        assert_eq!(id, 1);
        assert_eq!(username, "alice");
        assert_eq!(email, "alice@example.com");
    }

    #[test]
    fn test_parse_select_with_columns() {
        let result = parse_select("SELECT id, username FROM users");
        assert!(result.is_ok());
        let (cols, where_clause) = result.unwrap();
        assert!(cols.is_some());
        assert_eq!(cols.unwrap().len(), 2);
        assert!(where_clause.is_none());
    }

    #[test]
    fn test_parse_select_where_simple() {
        let result = parse_select("SELECT * WHERE id = 1");
        assert!(result.is_ok());
        let (cols, where_clause) = result.unwrap();
        assert!(cols.is_none());
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(operators.len(), 0);
    }

    #[test]
    fn test_parse_select_where_and() {
        let result = parse_select("SELECT WHERE id > 1 AND username = alice");
        assert!(result.is_ok());
        let (_, where_clause) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(operators.len(), 1);
        assert_eq!(operators[0], "AND");
    }

    #[test]
    fn test_parse_select_where_or() {
        let result = parse_select("SELECT WHERE id = 1 OR username = bob");
        assert!(result.is_ok());
        let (_, where_clause) = result.unwrap();
        let (_, operators) = where_clause.unwrap();
        assert_eq!(operators[0], "OR");
    }

    #[test]
    fn test_parse_select_where_mixed() {
        let result = parse_select("SELECT WHERE id > 2 AND username = alice OR id = 3");
        assert!(result.is_ok());
        let (_, where_clause) = result.unwrap();
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 3);
        assert_eq!(operators.len(), 2);
        assert_eq!(operators[0], "AND");
        assert_eq!(operators[1], "OR");
    }

    #[test]
    fn test_parse_update() {
        let result = parse_update("UPDATE users SET username = 'newname' WHERE id = 1");
        assert!(result.is_ok());
        let (id, column, value) = result.unwrap();
        assert_eq!(id, 1);
        assert_eq!(column, "username");
        assert_eq!(value, "newname");
    }

    #[test]
    fn test_parse_delete() {
        let result = parse_delete("DELETE FROM users WHERE id = 5");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }

    #[test]
    fn test_parse_delete_where() {
        let result = parse_delete_where("DELETE WHERE username = 'alice'");
        assert!(result.is_ok());
        let (column, value) = result.unwrap();
        assert_eq!(column, "username");
        assert_eq!(value, "alice");
    }

    #[test]
    fn test_parse_operators_all() {
        let result = parse_select("SELECT WHERE id > 1");
        assert!(result.is_ok());

        let result = parse_select("SELECT WHERE id < 10");
        assert!(result.is_ok());

        let result = parse_select("SELECT WHERE id >= 5");
        assert!(result.is_ok());

        let result = parse_select("SELECT WHERE id <= 5");
        assert!(result.is_ok());

        let result = parse_select("SELECT WHERE id != 3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_insert() {
        let result = parse_insert("INSERT invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_select() {
        let result = parse_select("INVALID QUERY");
        assert!(result.is_err());
    }
}
