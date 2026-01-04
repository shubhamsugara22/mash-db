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
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(!distinct);
        assert!(cols.is_some());
        assert_eq!(cols.unwrap().len(), 2);
        assert!(where_clause.is_none());
        assert!(order_by.is_none());
        assert!(limit.is_none());
        assert!(offset.is_none());
    }

    #[test]
    fn test_parse_select_where_simple() {
        let result = parse_select("SELECT * WHERE id = 1");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, _, _, _) = result.unwrap();
        assert!(!distinct);
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
        let (_, _, where_clause, _, _, _, _) = result.unwrap();
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
        let (_, _, where_clause, _, _, _, _) = result.unwrap();
        let (_, operators) = where_clause.unwrap();
        assert_eq!(operators[0], "OR");
    }

    #[test]
    fn test_parse_select_where_mixed() {
        let result = parse_select("SELECT WHERE id > 2 AND username = alice OR id = 3");
        assert!(result.is_ok());
        let (_, _, where_clause, _, _, _, _) = result.unwrap();
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

    #[test]
    fn test_parse_select_with_order_by_asc() {
        let result = parse_select("SELECT * WHERE id > 1 ORDER BY username ASC");
        assert!(result.is_ok());
        let (_, _, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(where_clause.is_some());
        assert!(order_by.is_some());
        let (col, is_asc) = order_by.unwrap();
        assert_eq!(col, "username");
        assert!(is_asc);
        assert!(limit.is_none());
        assert!(offset.is_none());
    }

    #[test]
    fn test_parse_select_with_order_by_desc() {
        let result = parse_select("SELECT * ORDER BY id DESC");
        assert!(result.is_ok());
        let (_, _, _, _, order_by, _, _) = result.unwrap();
        assert!(order_by.is_some());
        let (col, is_asc) = order_by.unwrap();
        assert_eq!(col, "id");
        assert!(!is_asc);
    }

    #[test]
    fn test_parse_select_with_limit() {
        let result = parse_select("SELECT * ORDER BY username LIMIT 10");
        assert!(result.is_ok());
        let (_, _, _, _, order_by, limit, offset) = result.unwrap();
        assert!(order_by.is_some());
        assert_eq!(limit, Some(10));
        assert!(offset.is_none());
    }

    #[test]
    fn test_parse_select_with_offset() {
        let result = parse_select("SELECT * LIMIT 5 OFFSET 20");
        assert!(result.is_ok());
        let (_, _, _, _, _, limit, offset) = result.unwrap();
        assert_eq!(limit, Some(5));
        assert_eq!(offset, Some(20));
    }

    #[test]
    fn test_parse_select_full_clause() {
        let result = parse_select("SELECT id, username WHERE email = test@test.com ORDER BY username DESC LIMIT 15 OFFSET 5");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(!distinct);
        assert!(cols.is_some());
        assert!(where_clause.is_some());
        assert!(order_by.is_some());
        let (col, is_asc) = order_by.unwrap();
        assert_eq!(col, "username");
        assert!(!is_asc);
        assert_eq!(limit, Some(15));
        assert_eq!(offset, Some(5));
    }

    #[test]
    fn test_parse_select_distinct_star() {
        let result = parse_select("SELECT DISTINCT * FROM users");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(distinct);
        assert!(cols.is_none());
        assert!(where_clause.is_none());
        assert!(order_by.is_none());
        assert_eq!(limit, None);
        assert_eq!(offset, None);
    }

    #[test]
    fn test_parse_select_distinct_columns() {
        let result = parse_select("SELECT DISTINCT username, email FROM users");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(distinct);
        assert!(cols.is_some());
        let columns = cols.unwrap();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0], "username");
        assert_eq!(columns[1], "email");
        assert!(where_clause.is_none());
        assert!(order_by.is_none());
    }

    #[test]
    fn test_parse_select_distinct_with_where() {
        let result = parse_select("SELECT DISTINCT id WHERE username = alice");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(distinct);
        assert!(cols.is_some());
        assert!(where_clause.is_some());
    }

    #[test]
    fn test_parse_select_distinct_full_clause() {
        let result =
            parse_select("SELECT DISTINCT username WHERE id > 5 ORDER BY username ASC LIMIT 10");
        assert!(result.is_ok());
        let (distinct, cols, where_clause, _, order_by, limit, offset) = result.unwrap();
        assert!(distinct);
        assert!(cols.is_some());
        assert!(where_clause.is_some());
        assert!(order_by.is_some());
        assert_eq!(limit, Some(10));
    }

    #[test]
    fn test_parse_select_group_by_single() {
        let result = parse_select("SELECT username GROUP BY username");
        assert!(result.is_ok());
        let (_, cols, _, group_by, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(group_by.is_some());
        let groups = group_by.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], "username");
    }

    #[test]
    fn test_parse_select_group_by_multiple() {
        let result = parse_select("SELECT username, email GROUP BY username, email");
        assert!(result.is_ok());
        let (_, cols, _, group_by, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(group_by.is_some());
        let groups = group_by.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], "username");
        assert_eq!(groups[1], "email");
    }

    #[test]
    fn test_parse_select_group_by_with_where() {
        let result = parse_select("SELECT username WHERE id > 1 GROUP BY username");
        assert!(result.is_ok());
        let (_, cols, where_clause, group_by, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(where_clause.is_some());
        assert!(group_by.is_some());
    }

    #[test]
    fn test_parse_select_group_by_with_order() {
        let result = parse_select("SELECT username GROUP BY username ORDER BY username");
        assert!(result.is_ok());
        let (_, _, _, group_by, order_by, _, _) = result.unwrap();
        assert!(group_by.is_some());
        assert!(order_by.is_some());
    }
}
