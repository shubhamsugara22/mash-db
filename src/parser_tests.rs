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
        let (table_name, values) = result.unwrap();
        assert!(table_name.is_none());
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "alice");
        assert_eq!(values[2], "alice@example.com");
    }

    #[test]
    fn test_parse_insert_full_format() {
        let result = parse_insert("INSERT INTO users VALUES (1, 'alice', 'alice@example.com')");
        assert!(result.is_ok());
        let (table_name, values) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "alice");
        assert_eq!(values[2], "alice@example.com");
    }

    #[test]
    fn test_parse_insert_full_format_with_unquoted_float() {
        let result = parse_insert("INSERT INTO products VALUES (1, Widget, 19.99, Tools)");
        assert!(result.is_ok());
        let (table_name, values) = result.unwrap();
        assert_eq!(table_name, Some("products".to_string()));
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "Widget");
        assert_eq!(values[2], "19.99");
        assert_eq!(values[3], "Tools");
    }

    #[test]
    fn test_parse_select_where_with_unquoted_float() {
        let result = parse_select("SELECT * FROM products WHERE price > 19.99");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(operators.len(), 0);
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "price");
        assert_eq!(conditions[0].1, ">");
        assert_eq!(conditions[0].2, "19.99");
    }

    #[test]
    fn test_parse_select_where_with_signed_number() {
        let result = parse_select("SELECT * FROM products WHERE delta <= -12.5");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(operators.len(), 0);
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "delta");
        assert_eq!(conditions[0].1, "<=");
        assert_eq!(conditions[0].2, "-12.5");
    }

    #[test]
    fn test_parse_insert_with_scientific_notation() {
        let result = parse_insert("INSERT INTO metrics VALUES (1, load, 1.25e3)");
        assert!(result.is_ok());
        let (table_name, values) = result.unwrap();
        assert_eq!(table_name, Some("metrics".to_string()));
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "load");
        assert_eq!(values[2], "1.25e3");
    }

    #[test]
    fn test_tokenize_scientific_and_signed_literals() {
        let tokens = tokenize("value >= -2.5E-3 AND value < +1e6");
        assert_eq!(tokens[0], Token::Identifier("value".to_string()));
        assert_eq!(tokens[1], Token::Ge);
        assert_eq!(tokens[2], Token::String("-2.5E-3".to_string()));
        assert_eq!(tokens[3], Token::And);
        assert_eq!(tokens[4], Token::Identifier("value".to_string()));
        assert_eq!(tokens[5], Token::Lt);
        assert_eq!(tokens[6], Token::String("+1e6".to_string()));
    }

    #[test]
    fn test_tokenize_unsigned_leading_dot_literal() {
        let tokens = tokenize("reading >= .5");
        assert_eq!(tokens[0], Token::Identifier("reading".to_string()));
        assert_eq!(tokens[1], Token::Ge);
        assert_eq!(tokens[2], Token::String(".5".to_string()));
    }

    #[test]
    fn test_parse_select_where_with_unsigned_leading_dot_literal() {
        let result = parse_select("SELECT * FROM metrics WHERE reading < .5e2");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(operators.len(), 0);
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "reading");
        assert_eq!(conditions[0].1, "<");
        assert_eq!(conditions[0].2, ".5e2");
    }

    #[test]
    fn test_parse_select_with_columns() {
        let result = parse_select("SELECT id, username FROM users");
        assert!(result.is_ok());
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (distinct, cols, _, _, where_clause, _, _, _, _, _) = result.unwrap();
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
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
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
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (_, operators) = where_clause.unwrap();
        assert_eq!(operators[0], "OR");
    }

    #[test]
    fn test_parse_select_where_mixed() {
        let result = parse_select("SELECT WHERE id > 2 AND username = alice OR id = 3");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
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
        let (table_name, id, column, value) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
        assert_eq!(id, 1);
        assert_eq!(column, "username");
        assert_eq!(value, "newname");
    }

    #[test]
    fn test_parse_delete() {
        let result = parse_delete("DELETE FROM users WHERE id = 5");
        assert!(result.is_ok());
        let (table_name, id) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
        assert_eq!(id, 5);
    }

    #[test]
    fn test_parse_delete_where() {
        let result = parse_delete_where("DELETE FROM users WHERE username = 'alice'");
        assert!(result.is_ok());
        let (table_name, column, value) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
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
        let (_, _, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (_, _, _, _, _, _, _, order_by, _, _) = result.unwrap();
        assert!(order_by.is_some());
        let (col, is_asc) = order_by.unwrap();
        assert_eq!(col, "id");
        assert!(!is_asc);
    }

    #[test]
    fn test_parse_select_with_limit() {
        let result = parse_select("SELECT * ORDER BY username LIMIT 10");
        assert!(result.is_ok());
        let (_, _, _, _, _, _, _, order_by, limit, offset) = result.unwrap();
        assert!(order_by.is_some());
        assert_eq!(limit, Some(10));
        assert!(offset.is_none());
    }

    #[test]
    fn test_parse_select_with_offset() {
        let result = parse_select("SELECT * LIMIT 5 OFFSET 20");
        assert!(result.is_ok());
        let (_, _, _, _, _, _, _, _, limit, offset) = result.unwrap();
        assert_eq!(limit, Some(5));
        assert_eq!(offset, Some(20));
    }

    #[test]
    fn test_parse_select_full_clause() {
        let result = parse_select("SELECT id, username WHERE email = test@test.com ORDER BY username DESC LIMIT 15 OFFSET 5");
        assert!(result.is_ok());
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
        assert!(distinct);
        assert!(cols.is_some());
        assert!(where_clause.is_some());
    }

    #[test]
    fn test_parse_select_distinct_full_clause() {
        let result =
            parse_select("SELECT DISTINCT username WHERE id > 5 ORDER BY username ASC LIMIT 10");
        assert!(result.is_ok());
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, offset) = result.unwrap();
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
        let (_, cols, _, _, _, group_by, _, _, _, _) = result.unwrap();
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
        let (_, cols, _, _, _, group_by, _, _, _, _) = result.unwrap();
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
        let (_, cols, _, _, where_clause, group_by, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(where_clause.is_some());
        assert!(group_by.is_some());
    }

    #[test]
    fn test_parse_select_group_by_with_order() {
        let result = parse_select("SELECT username GROUP BY username ORDER BY username");
        assert!(result.is_ok());
        let (_, _, _, _, _, group_by, _, order_by, _, _) = result.unwrap();
        assert!(group_by.is_some());
        assert!(order_by.is_some());
    }

    #[test]
    fn test_parse_select_columns_star() {
        use crate::parser::parse_select_columns;
        let tokens = vec![Token::Star];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        matches!(col_vec[0], SelectColumn::Star);
    }

    #[test]
    fn test_parse_select_columns_regular() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Identifier("username".to_string()),
            Token::Comma,
            Token::Identifier("email".to_string()),
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 2);
    }

    #[test]
    fn test_parse_select_columns_count_star() {
        use crate::parser::parse_select_columns;
        let tokens = vec![Token::Count, Token::LParen, Token::Star, Token::RParen];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Count(None)) => assert!(true),
            _ => assert!(false, "Expected COUNT(*)"),
        }
    }

    #[test]
    fn test_parse_select_columns_count_column() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Count,
            Token::LParen,
            Token::Identifier("id".to_string()),
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Count(Some(col))) => assert_eq!(col, "id"),
            _ => assert!(false, "Expected COUNT(id)"),
        }
    }

    #[test]
    fn test_parse_select_columns_sum() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Sum,
            Token::LParen,
            Token::Identifier("age".to_string()),
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Sum(col)) => assert_eq!(col, "age"),
            _ => assert!(false, "Expected SUM(age)"),
        }
    }

    #[test]
    fn test_parse_select_columns_avg() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Avg,
            Token::LParen,
            Token::Identifier("salary".to_string()),
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Avg(col)) => assert_eq!(col, "salary"),
            _ => assert!(false, "Expected AVG(salary)"),
        }
    }

    #[test]
    fn test_parse_select_columns_min() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Min,
            Token::LParen,
            Token::Identifier("score".to_string()),
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Min(col)) => assert_eq!(col, "score"),
            _ => assert!(false, "Expected MIN(score)"),
        }
    }

    #[test]
    fn test_parse_select_columns_max() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Max,
            Token::LParen,
            Token::Identifier("score".to_string()),
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        match &col_vec[0] {
            SelectColumn::Aggregate(AggregateFunc::Max(col)) => assert_eq!(col, "score"),
            _ => assert!(false, "Expected MAX(score)"),
        }
    }

    #[test]
    fn test_parse_select_columns_mixed() {
        use crate::parser::parse_select_columns;
        let tokens = vec![
            Token::Identifier("username".to_string()),
            Token::Comma,
            Token::Count,
            Token::LParen,
            Token::Star,
            Token::RParen,
        ];
        let mut i = 0;
        let result = parse_select_columns(&tokens, &mut i);
        assert!(result.is_ok());
        let cols = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 2);
        match &col_vec[0] {
            SelectColumn::Column(col) => assert_eq!(col, "username"),
            _ => assert!(false, "Expected Column(username)"),
        }
        match &col_vec[1] {
            SelectColumn::Aggregate(AggregateFunc::Count(None)) => assert!(true),
            _ => assert!(false, "Expected COUNT(*)"),
        }
    }

    #[test]
    fn test_parse_select_with_count_star() {
        let result = parse_select("SELECT COUNT(*) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "count(*)");
    }

    #[test]
    fn test_parse_select_with_count_column() {
        let result = parse_select("SELECT COUNT(id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "count(id)");
    }

    #[test]
    fn test_parse_select_with_sum() {
        let result = parse_select("SELECT SUM(age) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "sum(age)");
    }

    #[test]
    fn test_parse_select_with_avg() {
        let result = parse_select("SELECT AVG(salary) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "avg(salary)");
    }

    #[test]
    fn test_parse_select_with_min() {
        let result = parse_select("SELECT MIN(score) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "min(score)");
    }

    #[test]
    fn test_parse_select_with_max() {
        let result = parse_select("SELECT MAX(score) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 1);
        assert_eq!(col_vec[0], "max(score)");
    }

    #[test]
    fn test_parse_select_mixed_regular_and_aggregate() {
        let result = parse_select("SELECT username, COUNT(*) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 2);
        assert_eq!(col_vec[0], "username");
        assert_eq!(col_vec[1], "count(*)");
    }

    #[test]
    fn test_parse_select_multiple_aggregates() {
        let result = parse_select("SELECT COUNT(*), SUM(age), AVG(salary) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 3);
        assert_eq!(col_vec[0], "count(*)");
        assert_eq!(col_vec[1], "sum(age)");
        assert_eq!(col_vec[2], "avg(salary)");
    }

    #[test]
    fn test_parse_select_with_aggregate_and_group_by() {
        let result = parse_select("SELECT username, COUNT(*) FROM users GROUP BY username");
        assert!(result.is_ok());
        let (_, cols, _, _, _, group_by, having, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(group_by.is_some());
        assert!(having.is_none());
        let col_vec = cols.unwrap();
        assert_eq!(col_vec.len(), 2);
        assert_eq!(col_vec[0], "username");
        assert_eq!(col_vec[1], "count(*)");
    }

    #[test]
    fn test_parse_select_with_having_simple() {
        let result = parse_select(
            "SELECT username, COUNT(*) FROM users GROUP BY username HAVING count(*) > 2",
        );
        assert!(result.is_ok());
        let (_, cols, _, _, _, group_by, having, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        assert!(group_by.is_some());
        assert!(having.is_some());

        let (conditions, operators) = having.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "count(*)");
        assert_eq!(conditions[0].1, ">");
        assert_eq!(conditions[0].2, "2");
        assert_eq!(operators.len(), 0);
    }

    #[test]
    fn test_parse_select_with_having_multiple_conditions() {
        let result = parse_select("SELECT username, COUNT(*), AVG(id) FROM users GROUP BY username HAVING count(*) > 2 AND avg(id) < 50");
        assert!(result.is_ok());
        let (_, _, _, _, _, group_by, having, _, _, _) = result.unwrap();
        assert!(group_by.is_some());
        assert!(having.is_some());

        let (conditions, operators) = having.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].0, "count(*)");
        assert_eq!(conditions[0].1, ">");
        assert_eq!(conditions[0].2, "2");
        assert_eq!(conditions[1].0, "avg(id)");
        assert_eq!(conditions[1].1, "<");
        assert_eq!(conditions[1].2, "50");
        assert_eq!(operators.len(), 1);
        assert_eq!(operators[0], "AND");
    }

    #[test]
    fn test_parse_select_with_having_or_operator() {
        let result = parse_select("SELECT username, SUM(id) FROM users GROUP BY username HAVING sum(id) > 100 OR sum(id) < 10");
        assert!(result.is_ok());
        let (_, _, _, _, _, _, having, _, _, _) = result.unwrap();
        assert!(having.is_some());

        let (conditions, operators) = having.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(operators.len(), 1);
        assert_eq!(operators[0], "OR");
    }

    #[test]
    fn test_parse_select_with_having_equality() {
        let result =
            parse_select("SELECT email, COUNT(*) FROM users GROUP BY email HAVING count(*) = 1");
        assert!(result.is_ok());
        let (_, _, _, _, _, _, having, _, _, _) = result.unwrap();
        assert!(having.is_some());

        let (conditions, _) = having.unwrap();
        assert_eq!(conditions[0].1, "=");
    }

    #[test]
    fn test_parse_select_with_having_and_order_by() {
        let result = parse_select("SELECT username, COUNT(*) FROM users GROUP BY username HAVING count(*) > 1 ORDER BY username ASC");
        assert!(result.is_ok());
        let (_, _, _, _, _, group_by, having, order_by, _, _) = result.unwrap();
        assert!(group_by.is_some());
        assert!(having.is_some());
        assert!(order_by.is_some());

        let (col, is_asc) = order_by.unwrap();
        assert_eq!(col, "username");
        assert!(is_asc);
    }

    #[test]
    fn test_parse_select_with_having_and_limit() {
        let result = parse_select(
            "SELECT username, COUNT(*) FROM users GROUP BY username HAVING count(*) > 1 LIMIT 5",
        );
        assert!(result.is_ok());
        let (_, _, _, _, _, _, having, _, limit, _) = result.unwrap();
        assert!(having.is_some());
        assert_eq!(limit, Some(5));
    }

    #[test]
    fn test_tokenize_having() {
        let tokens = tokenize("HAVING count(*) > 2");
        assert!(tokens.iter().any(|t| matches!(t, Token::Having)));
    }

    #[test]
    fn test_parse_count_distinct_single() {
        let result = parse_select("SELECT COUNT(DISTINCT username) FROM users");
        assert!(result.is_ok());
        let (distinct, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(!distinct);
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0], "count(distinct username)");
    }

    #[test]
    fn test_parse_count_distinct_with_group_by() {
        let result =
            parse_select("SELECT email, COUNT(DISTINCT username) FROM users GROUP BY email");
        assert!(result.is_ok());
        let (_, cols, _, _, _, group_by, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0], "email");
        assert_eq!(cols[1], "count(distinct username)");
        assert_eq!(group_by, Some(vec!["email".to_string()]));
    }

    #[test]
    fn test_parse_count_distinct_multiple() {
        let result =
            parse_select("SELECT COUNT(DISTINCT username), COUNT(DISTINCT email) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0], "count(distinct username)");
        assert_eq!(cols[1], "count(distinct email)");
    }

    #[test]
    fn test_parse_select_with_from_and_inner_join() {
        let result = parse_select("SELECT * FROM users JOIN orders ON id = id");
        assert!(result.is_ok());
        let (distinct, cols, from_table, join, _, _, _, _, _, _) = result.unwrap();
        assert!(!distinct);
        assert!(cols.is_none());
        assert_eq!(from_table, Some("users".to_string()));
        assert!(join.is_some());
        let jc = join.unwrap();
        assert_eq!(jc.table, "orders");
        assert_eq!(jc.on_left, "id");
        assert_eq!(jc.on_right, "id");
        assert_eq!(jc.join_type, crate::parser::JoinType::Inner);
    }

    #[test]
    fn test_parse_select_with_left_join() {
        let result = parse_select("SELECT * FROM users LEFT JOIN orders ON username = username");
        assert!(result.is_ok());
        let (_, _, from_table, join, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(from_table, Some("users".to_string()));
        assert!(join.is_some());
        let jc = join.unwrap();
        assert_eq!(jc.table, "orders");
        assert_eq!(jc.on_left, "username");
        assert_eq!(jc.on_right, "username");
        assert_eq!(jc.join_type, crate::parser::JoinType::Left);
    }

    #[test]
    fn test_parse_count_distinct_star_should_fail() {
        let result = parse_select("SELECT COUNT(DISTINCT *) FROM users");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("COUNT(DISTINCT *) is not supported"));
    }
}
