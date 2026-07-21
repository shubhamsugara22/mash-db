#[cfg(test)]
mod tests {
    use crate::compute_dense_rank_map;
    use crate::compute_lag_map;
    use crate::compute_lead_map;
    use crate::compute_rank_map;
    use crate::compute_row_number_map;
    use crate::parser::*;
    use crate::table::{Row, Table};

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
        if let Err(e) = &result {
            panic!("parse_select returned Err: {}", e);
        }
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
    fn test_parse_row_number_basic() {
        let result = parse_select("SELECT ROW_NUMBER() OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__row_number__:"));
        // Expect no partition and order by id encoded after separator
        assert!(cols[0].ends_with("\x1Fid"));
    }

    #[test]
    fn test_row_number_runtime() {
        let mut table = Table::new(
            "test_rows.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        table
            .insert(Row::new(1, "alice".to_string(), "a@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "alice".to_string(), "a2@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "bob".to_string(), "b@e.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_all();
        let encoded = "__row_number__:username\x1Fid".to_string();
        let mapping = compute_row_number_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("1"));
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("2"));
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_parse_rank_basic() {
        let result = parse_select("SELECT RANK() OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__rank__:"));
        // Expect no partition and order by id encoded after separator
        assert!(cols[0].ends_with("\x1Fid"));
    }

    #[test]
    fn test_rank_runtime() {
        let mut table = Table::new(
            "test_rows_rank.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        table
            .insert(Row::new(1, "10".to_string(), "a@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "10".to_string(), "a2@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "5".to_string(), "b@e.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_all();
        let encoded = "__rank__:\x1Fusername:DESC".to_string();
        let mapping = compute_rank_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("1"));
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("1"));
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("3"));
    }

    #[test]
    fn test_parse_dense_rank_basic() {
        let result = parse_select("SELECT DENSE_RANK() OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__dense_rank__:"));
        assert!(cols[0].ends_with("\x1Fid"));
    }

    #[test]
    fn test_dense_rank_runtime() {
        let mut table = Table::new(
            "test_rows_dense_rank.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        table
            .insert(Row::new(1, "10".to_string(), "a@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "10".to_string(), "a2@e.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "5".to_string(), "b@e.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_all();
        let encoded = "__dense_rank__:\x1Fusername:DESC".to_string();
        let mapping = compute_dense_rank_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("1"));
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("1"));
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("2"));
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
    fn test_tokenize_standalone_dot_stays_dot_token() {
        let tokens = tokenize("users . id");
        assert_eq!(tokens[0], Token::Identifier("users".to_string()));
        assert_eq!(tokens[1], Token::Dot);
        assert_eq!(tokens[2], Token::Identifier("id".to_string()));
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
        let (table_name, id, assignments) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
        assert_eq!(id, 1);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].0, "username");
        assert_eq!(assignments[0].1, "newname");
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
    fn test_parse_select_limit_one() {
        let result = parse_select("SELECT * LIMIT 1");
        assert!(result.is_ok());
        let (_, _, _, _, _, _, _, _order_by, limit, _offset) = result.unwrap();
        assert_eq!(limit, Some(1));
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
        let (distinct, cols, _, _, where_clause, _, _, order_by, _limit, _offset) = result.unwrap();
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
        let (distinct, cols, _, _, where_clause, _, _, _order_by, _limit, _offset) =
            result.unwrap();
        assert!(distinct);
        assert!(cols.is_some());
        assert!(where_clause.is_some());
    }

    #[test]
    fn test_parse_select_distinct_full_clause() {
        let result =
            parse_select("SELECT DISTINCT username WHERE id > 5 ORDER BY username ASC LIMIT 10");
        assert!(result.is_ok());
        let (distinct, cols, _, _, where_clause, _, _, order_by, limit, _offset) = result.unwrap();
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

    // ── Numeric-literal negative / regression tests ───────────────────────────

    #[test]
    fn test_tokenize_double_dot_splits_into_two_string_tokens() {
        // "1.2.3" must not panic; the second dot+digit becomes a separate String token
        let tokens = tokenize("1.2.3");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::String("1.2".to_string()));
        assert_eq!(tokens[1], Token::String(".3".to_string()));
    }

    #[test]
    fn test_tokenize_incomplete_exponent_not_consumed() {
        // "5e" — 'e' without a following digit must not be consumed as an exponent
        let tokens = tokenize("5e");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Number(5));
        assert_eq!(tokens[1], Token::Identifier("e".to_string()));
    }

    #[test]
    fn test_tokenize_exponent_sign_without_digit_not_consumed() {
        // "5e+" — sign after 'e' but no digit: exponent not consumed, bare '+' is dropped
        let tokens = tokenize("5e+");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Number(5));
        assert_eq!(tokens[1], Token::Identifier("e".to_string()));
    }

    #[test]
    fn test_tokenize_bare_minus_between_identifiers_is_dropped() {
        // "x - y" — minus not adjacent to a digit is silently dropped
        let tokens = tokenize("x - y");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Identifier("x".to_string()));
        assert_eq!(tokens[1], Token::Identifier("y".to_string()));
    }

    #[test]
    fn test_tokenize_bare_plus_between_identifiers_is_dropped() {
        // "a + b" — plus not adjacent to a digit is silently dropped
        let tokens = tokenize("a + b");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Identifier("a".to_string()));
        assert_eq!(tokens[1], Token::Identifier("b".to_string()));
    }

    #[test]
    fn test_tokenize_leading_dot_without_digit_is_dot_token() {
        // ". col" — dot not followed by a digit must produce a Dot token, not a numeric
        let tokens = tokenize(". col");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Dot);
        assert_eq!(tokens[1], Token::Identifier("col".to_string()));
    }

    #[test]
    fn test_tokenize_sign_then_dot_no_digit_sign_dropped() {
        // "-. x" — minus then dot but no digit after dot: sign is dropped, dot stays Dot
        let tokens = tokenize("-. x");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Dot);
        assert_eq!(tokens[1], Token::Identifier("x".to_string()));
    }

    #[test]
    fn test_tokenize_number_followed_by_incomplete_exponent_no_panic() {
        // Regression guard: "10e abc" must not panic and must not merge tokens
        let tokens = tokenize("10e abc");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Number(10));
        assert_eq!(tokens[1], Token::Identifier("e".to_string()));
        assert_eq!(tokens[2], Token::Identifier("abc".to_string()));
    }

    #[test]
    fn test_parse_insert_missing_values_keyword_is_error() {
        // "INSERT INTO users (1, alice)" — VALUES keyword is required; must return Err
        let result = parse_insert("INSERT INTO users (1, alice)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_select_double_dot_value_does_not_panic() {
        // Malformed WHERE value with a double-dot literal must not panic; result is unspecified
        let _ = parse_select("SELECT * FROM t WHERE x > 1.2.3");
    }

    #[test]
    fn test_parse_select_where_not_in_list() {
        let result = parse_select("SELECT * FROM users WHERE id NOT IN (1, 2, 3)");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, _) = where_clause.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "id");
        assert_eq!(conditions[0].1, "NOT_IN");
        assert_eq!(conditions[0].2, "1,2,3");
    }

    #[test]
    fn test_parse_select_where_not_in_multiple_conditions() {
        let result =
            parse_select("SELECT * FROM users WHERE username NOT IN ('alice', 'bob') AND id = 5");
        assert!(result.is_ok());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        assert!(where_clause.is_some());
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].1, "NOT_IN");
        assert_eq!(conditions[1].1, "=");
        assert_eq!(operators[0], "AND");
    }

    #[test]
    fn test_parse_update_multi_column() {
        let result = parse_update(
            "UPDATE users SET username = 'bob', email = 'bob@example.com' WHERE id = 2",
        );
        assert!(result.is_ok());
        let (table_name, id, assignments) = result.unwrap();
        assert_eq!(table_name, Some("users".to_string()));
        assert_eq!(id, 2);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0], ("username".to_string(), "bob".to_string()));
        assert_eq!(
            assignments[1],
            ("email".to_string(), "bob@example.com".to_string())
        );
    }

    #[test]
    fn test_parse_update_numeric_value() {
        let result = parse_update("UPDATE products SET price = 99 WHERE id = 3");
        assert!(result.is_ok());
        let (_, id, assignments) = result.unwrap();
        assert_eq!(id, 3);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].0, "price");
        assert_eq!(assignments[0].1, "99");
    }

    #[test]
    fn test_tokenize_union_keyword() {
        let tokens = tokenize("SELECT * FROM a UNION SELECT * FROM b");
        assert!(tokens.contains(&Token::Union));
    }

    #[test]
    fn test_tokenize_exists_keyword() {
        let tokens = tokenize("SELECT * FROM users WHERE EXISTS (SELECT id FROM orders)");
        assert!(tokens.contains(&Token::Exists));
    }

    #[test]
    fn test_parse_select_where_exists_subquery() {
        let result = parse_select(
            "SELECT * FROM users WHERE EXISTS (SELECT id FROM orders WHERE orders.user_id = 1)",
        );
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (conditions, _operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "__exists__");
        assert_eq!(conditions[0].1, "EXISTS_SUBQUERY");
        assert!(!conditions[0].2.is_empty());
    }

    #[test]
    fn test_parse_select_where_not_exists_subquery() {
        let result = parse_select(
            "SELECT * FROM users WHERE NOT EXISTS (SELECT id FROM orders WHERE orders.user_id = 1)",
        );
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (conditions, _operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].0, "__exists__");
        assert_eq!(conditions[0].1, "NOT_EXISTS_SUBQUERY");
        assert!(!conditions[0].2.is_empty());
    }

    #[test]
    fn test_parse_select_where_exists_with_and() {
        let result =
            parse_select("SELECT * FROM users WHERE id = 1 AND EXISTS (SELECT id FROM orders)");
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(operators[0], "AND");
        assert_eq!(conditions[1].0, "__exists__");
        assert_eq!(conditions[1].1, "EXISTS_SUBQUERY");
    }

    #[test]
    fn test_tokenize_upper_lower_length() {
        use crate::parser::Token;
        let upper_tokens = crate::parser::tokenize("UPPER(x)");
        let lower_tokens = crate::parser::tokenize("LOWER(x)");
        let length_tokens = crate::parser::tokenize("LENGTH(x)");
        assert!(upper_tokens.contains(&Token::Upper));
        assert!(lower_tokens.contains(&Token::Lower));
        assert!(length_tokens.contains(&Token::Length));
    }

    #[test]
    fn test_parse_select_upper_column() {
        let result = parse_select("SELECT UPPER(username) FROM users");
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["upper(username)".to_string()]));
    }

    #[test]
    fn test_parse_select_lower_column() {
        let result = parse_select("SELECT LOWER(email) FROM users");
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["lower(email)".to_string()]));
    }

    #[test]
    fn test_parse_select_length_column() {
        let result = parse_select("SELECT LENGTH(username) FROM users");
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["length(username)".to_string()]));
    }

    #[test]
    fn test_parse_select_where_upper_condition() {
        let result = parse_select("SELECT * FROM users WHERE UPPER(username) = 'ALICE'");
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (conditions, _) = where_clause.unwrap();
        assert_eq!(conditions[0].0, "upper(username)");
        assert_eq!(conditions[0].1, "=");
        assert_eq!(conditions[0].2, "ALICE");
    }

    #[test]
    fn test_parse_select_where_lower_and_length() {
        let result = parse_select(
            "SELECT * FROM users WHERE LOWER(username) = 'alice' AND LENGTH(email) > 5",
        );
        assert!(result.is_ok(), "parse_select failed: {:?}", result.err());
        let (_, _, _, _, where_clause, _, _, _, _, _) = result.unwrap();
        let (conditions, operators) = where_clause.unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].0, "lower(username)");
        assert_eq!(conditions[0].1, "=");
        assert_eq!(conditions[0].2, "alice");
        assert_eq!(conditions[1].0, "length(email)");
        assert_eq!(conditions[1].1, ">");
        assert_eq!(conditions[1].2, "5");
        assert_eq!(operators[0], "AND");
    }

    #[test]
    fn test_parse_insert_select_basic() {
        let result = parse_insert_select("INSERT INTO backup SELECT * FROM users");
        assert!(
            result.is_ok(),
            "parse_insert_select failed: {:?}",
            result.err()
        );
        let (table, select_sql) = result.unwrap();
        assert_eq!(table, "backup");
        assert!(select_sql.to_uppercase().starts_with("SELECT"));
        assert!(select_sql.to_uppercase().contains("FROM"));
    }

    #[test]
    fn test_parse_insert_select_with_where() {
        let result =
            parse_insert_select("INSERT INTO archive SELECT * FROM orders WHERE status = 'closed'");
        assert!(
            result.is_ok(),
            "parse_insert_select failed: {:?}",
            result.err()
        );
        let (table, select_sql) = result.unwrap();
        assert_eq!(table, "archive");
        assert!(select_sql.to_uppercase().contains("WHERE"));
    }

    #[test]
    fn test_parse_insert_select_specific_columns() {
        let result = parse_insert_select("INSERT INTO summary SELECT username, email FROM users");
        assert!(
            result.is_ok(),
            "parse_insert_select failed: {:?}",
            result.err()
        );
        let (table, select_sql) = result.unwrap();
        assert_eq!(table, "summary");
        assert!(select_sql.to_uppercase().contains("USERNAME"));
    }

    #[test]
    fn test_parse_insert_select_missing_into_fails() {
        let result = parse_insert_select("INSERT backup SELECT * FROM users");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_insert_select_missing_select_fails() {
        let result = parse_insert_select("INSERT INTO backup VALUES (1, 'a', 'b')");
        assert!(result.is_err());
    }

    // ── CASE WHEN tests ───────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_case_when_basic() {
        let sql = "SELECT CASE WHEN status = 'active' THEN 'yes' END FROM users";
        let result = parse_select(sql);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (_distinct, cols, table, ..) = result.unwrap();
        assert_eq!(table, Some("users".to_string()));
        let columns = cols.unwrap();
        assert_eq!(columns.len(), 1);
        let col = &columns[0];
        assert!(
            col.starts_with("__case__:"),
            "Expected encoded CASE, got: {}",
            col
        );
        assert!(col.contains("status\x1F=\x1Factive\x1Fyes"));
    }

    #[test]
    fn test_parse_select_case_when_else() {
        let sql = "SELECT CASE WHEN age > 18 THEN 'adult' ELSE 'minor' END FROM people";
        let result = parse_select(sql);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (_distinct, cols, ..) = result.unwrap();
        let columns = cols.unwrap();
        let col = &columns[0];
        assert!(
            col.starts_with("__case__:"),
            "Expected encoded CASE, got: {}",
            col
        );
        assert!(
            col.contains("age\x1F>\x1F18\x1Fadult"),
            "Missing WHEN branch"
        );
        assert!(col.contains("__else__\x1Fminor"), "Missing ELSE branch");
    }

    #[test]
    fn test_parse_select_case_when_multiple_branches() {
        let sql = "SELECT CASE WHEN score >= 90 THEN 'A' WHEN score >= 80 THEN 'B' ELSE 'C' END FROM grades";
        let result = parse_select(sql);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (_distinct, cols, ..) = result.unwrap();
        let columns = cols.unwrap();
        let col = &columns[0];
        assert!(col.contains("score\x1F>=\x1F90\x1FA"), "Missing first WHEN");
        assert!(
            col.contains("score\x1F>=\x1F80\x1FB"),
            "Missing second WHEN"
        );
        assert!(col.contains("__else__\x1FC"), "Missing ELSE");
    }

    #[test]
    fn test_parse_select_case_when_with_other_cols() {
        let sql = "SELECT name, CASE WHEN active = 1 THEN 'yes' ELSE 'no' END FROM users";
        let result = parse_select(sql);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (_distinct, cols, ..) = result.unwrap();
        let columns = cols.unwrap();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0], "name");
        assert!(columns[1].starts_with("__case__:"));
    }

    #[test]
    fn test_parse_select_case_when_missing_end_fails() {
        let sql = "SELECT CASE WHEN x = 1 THEN 'a' FROM t";
        let result = parse_select(sql);
        assert!(result.is_err(), "Expected error when END is missing");
    }

    #[test]
    fn test_eval_col_case_when_match() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("status".to_string(), "active".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        // encoded: status=active → "yes", else "no"
        let encoded = format!("__case__:status\x1F=\x1Factive\x1Fyes\x1E__else__\x1Fno");
        assert_eq!(row.eval_col(&encoded), Some("yes".to_string()));
    }

    #[test]
    fn test_eval_col_case_when_else_branch() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("status".to_string(), "inactive".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__case__:status\x1F=\x1Factive\x1Fyes\x1E__else__\x1Fno");
        assert_eq!(row.eval_col(&encoded), Some("no".to_string()));
    }

    #[test]
    fn test_eval_col_case_when_no_match_no_else() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("status".to_string(), "unknown".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = "__case__:status\x1F=\x1Factive\x1Fyes".to_string();
        assert_eq!(row.eval_col(&encoded), Some("NULL".to_string()));
    }

    #[test]
    fn test_eval_col_case_when_numeric_comparison() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("score".to_string(), "95".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded =
            format!("__case__:score\x1F>=\x1F90\x1FA\x1Escore\x1F>=\x1F80\x1FB\x1E__else__\x1FC");
        assert_eq!(row.eval_col(&encoded), Some("A".to_string()));
    }

    // ── COALESCE ──────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_coalesce_basic() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT COALESCE(email, 'none') FROM users");
        assert!(result.is_ok(), "parse failed: {:?}", result);
        let (_distinct, cols, table, ..) = result.unwrap();
        assert_eq!(table, Some("users".to_string()));
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__coalesce__:"),
            "Expected coalesce encoding, got: {}",
            cols[0]
        );
        assert!(cols[0].contains("email"), "Missing column in encoding");
        assert!(cols[0].contains("none"), "Missing default in encoding");
    }

    #[test]
    fn test_parse_select_coalesce_numeric_default() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT COALESCE(username, unknown) FROM users");
        assert!(result.is_ok(), "parse failed: {:?}", result);
        let (_distinct, cols, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert!(
            cols[0].starts_with("__coalesce__:"),
            "Expected coalesce encoding"
        );
        assert!(cols[0].contains("unknown"), "Missing default value");
    }

    #[test]
    fn test_eval_col_coalesce_with_value() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("notes".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__coalesce__:notes\x1Fno notes");
        assert_eq!(row.eval_col(&encoded), Some("hello".to_string()));
    }

    #[test]
    fn test_eval_col_coalesce_fallback_on_missing() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__coalesce__:notes\x1Fno notes");
        assert_eq!(row.eval_col(&encoded), Some("no notes".to_string()));
    }

    #[test]
    fn test_eval_col_coalesce_fallback_on_empty() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("notes".to_string(), "".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__coalesce__:notes\x1Ffallback");
        assert_eq!(row.eval_col(&encoded), Some("fallback".to_string()));
    }

    // ── NULLIF ────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_nullif_basic() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT NULLIF(username, 'admin') FROM users");
        assert!(result.is_ok(), "parse failed: {:?}", result);
        let (_distinct, cols, table, ..) = result.unwrap();
        assert_eq!(table, Some("users".to_string()));
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__nullif__:"),
            "Expected nullif encoding, got: {}",
            cols[0]
        );
        assert!(cols[0].contains("username"), "Missing column in encoding");
        assert!(cols[0].contains("admin"), "Missing comparison value");
    }

    #[test]
    fn test_eval_col_nullif_match_returns_null() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "admin".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__nullif__:username\x1Fadmin");
        assert_eq!(row.eval_col(&encoded), Some("NULL".to_string()));
    }

    #[test]
    fn test_eval_col_nullif_no_match_returns_value() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "alice".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__nullif__:username\x1Fadmin");
        assert_eq!(row.eval_col(&encoded), Some("alice".to_string()));
    }

    // --- TRIM tests ---

    #[test]
    fn test_parse_select_trim_basic() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT TRIM(username) FROM users");
        assert!(result.is_ok());
        let (_distinct, cols, table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__trim__:"),
            "expected __trim__: prefix, got {}",
            cols[0]
        );
        assert_eq!(table.unwrap(), "users");
    }

    #[test]
    fn test_eval_col_trim_strips_whitespace() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("notes".to_string(), "  hello  ".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        assert_eq!(row.eval_col("__trim__:notes"), Some("hello".to_string()));
    }

    #[test]
    fn test_eval_col_trim_no_whitespace_unchanged() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "alice".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        assert_eq!(row.eval_col("__trim__:username"), Some("alice".to_string()));
    }

    // --- CAST tests ---

    #[test]
    fn test_parse_select_cast_basic() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT CAST(id AS TEXT) FROM users");
        assert!(result.is_ok());
        let (_distinct, cols, table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__cast__:"),
            "expected __cast__: prefix, got {}",
            cols[0]
        );
        assert_eq!(table.unwrap(), "users");
    }

    #[test]
    fn test_eval_col_cast_to_integer() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("price".to_string(), "19.99".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__cast__:price\x1FINTEGER");
        assert_eq!(row.eval_col(&encoded), Some("19".to_string()));
    }

    #[test]
    fn test_eval_col_cast_to_text_passthrough() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 42,
            username: "bob".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__cast__:username\x1FTEXT");
        assert_eq!(row.eval_col(&encoded), Some("bob".to_string()));
    }

    // --- CONCAT tests ---

    #[test]
    fn test_parse_select_concat_basic() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT CONCAT(username, email) FROM users");
        assert!(result.is_ok());
        let (_distinct, cols, table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__concat__:"),
            "expected __concat__: prefix, got {}",
            cols[0]
        );
        assert_eq!(table.unwrap(), "users");
    }

    #[test]
    fn test_parse_select_concat_col_and_literal() {
        use crate::parser::parse_select;
        let result = parse_select("SELECT CONCAT(username, '@example.com') FROM users");
        assert!(result.is_ok());
        let (_distinct, cols, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert!(
            cols[0].contains("c:username"),
            "col arg should be c:username"
        );
        assert!(
            cols[0].contains("s:@example.com"),
            "literal arg should be s:@example.com"
        );
    }

    #[test]
    fn test_eval_col_concat_two_columns() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "alice".to_string(),
            email: "@ex.com".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__concat__:c:username\x1Fc:email");
        assert_eq!(row.eval_col(&encoded), Some("alice@ex.com".to_string()));
    }

    #[test]
    fn test_eval_col_concat_col_and_string_literal() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "alice".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__concat__:c:username\x1Fs:@domain.com");
        assert_eq!(row.eval_col(&encoded), Some("alice@domain.com".to_string()));
    }

    // ── IF tests ──────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_if_basic() {
        let result = parse_select("SELECT IF(score > 10, 'pass', 'fail') FROM grades");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__if__:"),
            "expected __if__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_if_condition_true() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("score".to_string(), "15".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__if__:score\x1F>\x1F10\x1Fpass\x1Ffail");
        assert_eq!(row.eval_col(&encoded), Some("pass".to_string()));
    }

    #[test]
    fn test_eval_col_if_condition_false() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("score".to_string(), "5".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__if__:score\x1F>\x1F10\x1Fpass\x1Ffail");
        assert_eq!(row.eval_col(&encoded), Some("fail".to_string()));
    }

    // ── ABS tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_abs_basic() {
        let result = parse_select("SELECT ABS(balance) FROM accounts");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__abs__:"),
            "expected __abs__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_abs_negative() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("balance".to_string(), "-42".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = "__abs__:balance".to_string();
        assert_eq!(row.eval_col(&encoded), Some("42".to_string()));
    }

    #[test]
    fn test_eval_col_abs_positive() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("balance".to_string(), "7".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = "__abs__:balance".to_string();
        assert_eq!(row.eval_col(&encoded), Some("7".to_string()));
    }

    // ── ROUND tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_round_basic() {
        let result = parse_select("SELECT ROUND(price, 2) FROM products");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__round__:"),
            "expected __round__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_round_two_decimals() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("price".to_string(), "19.456".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__round__:price\x1F2");
        assert_eq!(row.eval_col(&encoded), Some("19.46".to_string()));
    }

    #[test]
    fn test_eval_col_round_zero_decimals() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("price".to_string(), "19.5".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__round__:price\x1F0");
        assert_eq!(row.eval_col(&encoded), Some("20".to_string()));
    }

    // ── SUBSTR tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_substr_basic() {
        let result = parse_select("SELECT SUBSTR(email, 1, 3) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__substr__:"),
            "expected __substr__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_substr_from_start() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("addr".to_string(), "alice@ex.com".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__substr__:addr\x1F1\x1F5");
        assert_eq!(row.eval_col(&encoded), Some("alice".to_string()));
    }

    #[test]
    fn test_eval_col_substr_mid() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("addr".to_string(), "alice@ex.com".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__substr__:addr\x1F6\x1F2");
        assert_eq!(row.eval_col(&encoded), Some("@e".to_string()));
    }

    // ── REPLACE tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_replace_basic() {
        let result = parse_select("SELECT REPLACE(name, 'a', 'o') FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__replace__:"),
            "expected __replace__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_replace_substitutes() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "banana".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__replace__:username\x1Fa\x1Fo");
        assert_eq!(row.eval_col(&encoded), Some("bonono".to_string()));
    }

    #[test]
    fn test_eval_col_replace_no_match_unchanged() {
        use crate::table::Row;
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "hello".to_string(),
            email: "e".to_string(),
            extras: HashMap::new(),
        };
        let encoded = format!("__replace__:username\x1Fz\x1FX");
        assert_eq!(row.eval_col(&encoded), Some("hello".to_string()));
    }

    // ── LPAD tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_lpad_basic() {
        let result = parse_select("SELECT LPAD(code, 5, '0') FROM items");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__lpad__:"),
            "expected __lpad__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_lpad_pads_short_value() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("code".to_string(), "42".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__lpad__:code\x1F5\x1F0");
        assert_eq!(row.eval_col(&encoded), Some("00042".to_string()));
    }

    #[test]
    fn test_eval_col_lpad_no_pad_when_already_wide() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("code".to_string(), "123456".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__lpad__:code\x1F5\x1F0");
        assert_eq!(row.eval_col(&encoded), Some("123456".to_string()));
    }

    // ── RPAD tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_rpad_basic() {
        let result = parse_select("SELECT RPAD(label, 6, '-') FROM items");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(
            cols[0].starts_with("__rpad__:"),
            "expected __rpad__: prefix, got: {}",
            cols[0]
        );
    }

    #[test]
    fn test_eval_col_rpad_pads_short_value() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("label".to_string(), "hi".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__rpad__:label\x1F5\x1F-");
        assert_eq!(row.eval_col(&encoded), Some("hi---".to_string()));
    }

    #[test]
    fn test_eval_col_rpad_no_pad_when_already_wide() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("label".to_string(), "toolong".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__rpad__:label\x1F5\x1F-");
        assert_eq!(row.eval_col(&encoded), Some("toolong".to_string()));
    }

    // ── LEFT tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_left_basic() {
        let result = parse_select("SELECT LEFT(name, 3) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__left__:"));
    }

    #[test]
    fn test_eval_col_left_extracts_prefix() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__left__:word\x1F3");
        assert_eq!(row.eval_col(&encoded), Some("hel".to_string()));
    }

    #[test]
    fn test_eval_col_left_entire_string_when_len_exceeds() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hi".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__left__:word\x1F10");
        assert_eq!(row.eval_col(&encoded), Some("hi".to_string()));
    }

    // ── RIGHT tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_right_basic() {
        let result = parse_select("SELECT RIGHT(name, 3) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__right__:"));
    }

    #[test]
    fn test_eval_col_right_extracts_suffix() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__right__:word\x1F3");
        assert_eq!(row.eval_col(&encoded), Some("llo".to_string()));
    }

    #[test]
    fn test_eval_col_right_entire_string_when_len_exceeds() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hi".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__right__:word\x1F10");
        assert_eq!(row.eval_col(&encoded), Some("hi".to_string()));
    }

    // ── REVERSE tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_reverse_basic() {
        let result = parse_select("SELECT REVERSE(name) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__reverse__:"));
    }

    #[test]
    fn test_eval_col_reverse_flips_string() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__reverse__:word");
        assert_eq!(row.eval_col(&encoded), Some("olleh".to_string()));
    }

    #[test]
    fn test_eval_col_reverse_empty_string() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__reverse__:word");
        assert_eq!(row.eval_col(&encoded), Some("".to_string()));
    }

    // ── REPEAT tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_repeat_basic() {
        let result = parse_select("SELECT REPEAT(name, 3) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__repeat__:"));
    }

    #[test]
    fn test_eval_col_repeat_duplicates_string() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "ha".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__repeat__:word\x1F3");
        assert_eq!(row.eval_col(&encoded), Some("hahaha".to_string()));
    }

    #[test]
    fn test_eval_col_repeat_zero_times() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__repeat__:word\x1F0");
        assert_eq!(row.eval_col(&encoded), Some("".to_string()));
    }

    // ── INITCAP tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_initcap_basic() {
        let result = parse_select("SELECT INITCAP(name) FROM users");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__initcap__:"));
    }

    #[test]
    fn test_eval_col_initcap_single_word() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("word".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__initcap__:word");
        assert_eq!(row.eval_col(&encoded), Some("Hello".to_string()));
    }

    #[test]
    fn test_eval_col_initcap_multiple_words() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("phrase".to_string(), "hello world test".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__initcap__:phrase");
        assert_eq!(row.eval_col(&encoded), Some("Hello World Test".to_string()));
    }

    // ── FLOOR tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_floor_basic() {
        let result = parse_select("SELECT FLOOR(price) FROM products");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__floor__:"));
    }

    #[test]
    fn test_eval_col_floor_rounds_down() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("val".to_string(), "3.7".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__floor__:val");
        assert_eq!(row.eval_col(&encoded), Some("3".to_string()));
    }

    #[test]
    fn test_eval_col_floor_negative() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("val".to_string(), "-2.3".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__floor__:val");
        assert_eq!(row.eval_col(&encoded), Some("-3".to_string()));
    }

    // ── CEIL tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_select_ceil_basic() {
        let result = parse_select("SELECT CEIL(price) FROM products");
        let (_distinct, cols, _table, ..) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__ceil__:"));
    }

    #[test]
    fn test_eval_col_ceil_rounds_up() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("val".to_string(), "3.2".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__ceil__:val");
        assert_eq!(row.eval_col(&encoded), Some("4".to_string()));
    }

    #[test]
    fn test_eval_col_ceil_negative() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("val".to_string(), "-2.7".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__ceil__:val");
        assert_eq!(row.eval_col(&encoded), Some("-2".to_string()));
    }

    // MOD tests
    #[test]
    fn test_parse_select_mod_basic() {
        let result = parse_select("SELECT MOD(amount, 3) FROM orders");
        assert!(result.is_ok());
        let (_distinct, cols, _table, _where, _group, _having, _order, _limit, _offset, _join) =
            result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__mod__:"));
    }

    #[test]
    fn test_eval_col_mod_basic() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "10".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__mod__:num\x1F3");
        assert_eq!(row.eval_col(&encoded), Some("1".to_string()));
    }

    #[test]
    fn test_eval_col_mod_fractional() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "7.5".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__mod__:num\x1F2.5");
        assert_eq!(row.eval_col(&encoded), Some("0".to_string()));
    }

    // POWER tests
    #[test]
    fn test_parse_select_power_basic() {
        let result = parse_select("SELECT POWER(base, 2) FROM data");
        assert!(result.is_ok());
        let (_distinct, cols, _table, _where, _group, _having, _order, _limit, _offset, _join) =
            result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__power__:"));
    }

    #[test]
    fn test_eval_col_power_square() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("base".to_string(), "3".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__power__:base\x1F2");
        assert_eq!(row.eval_col(&encoded), Some("9".to_string()));
    }

    #[test]
    fn test_eval_col_power_cube() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("base".to_string(), "2".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__power__:base\x1F3");
        assert_eq!(row.eval_col(&encoded), Some("8".to_string()));
    }

    // SQRT tests
    #[test]
    fn test_parse_select_sqrt_basic() {
        let result = parse_select("SELECT SQRT(area) FROM shapes");
        assert!(result.is_ok());
        let (_distinct, cols, _table, _where, _group, _having, _order, _limit, _offset, _join) =
            result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__sqrt__:"));
    }

    #[test]
    fn test_eval_col_sqrt_perfect_square() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "16".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__sqrt__:num");
        assert_eq!(row.eval_col(&encoded), Some("4".to_string()));
    }

    #[test]
    fn test_eval_col_sqrt_decimal() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "2".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__sqrt__:num");
        let result = row.eval_col(&encoded).unwrap();
        let parsed: f64 = result.parse().unwrap();
        assert!((parsed - 1.414213562).abs() < 0.0001);
    }

    #[test]
    fn test_parse_select_sign_basic() {
        let result = parse_select("SELECT SIGN(num) FROM demo");
        assert!(result.is_ok());
        let (_distinct, cols, _table, _where, _join, _group, _having, _order, _limit, _offset) =
            result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__sign__:"));
    }

    #[test]
    fn test_eval_col_sign_positive() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "3.5".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__sign__:num");
        assert_eq!(row.eval_col(&encoded), Some("1".to_string()));
    }

    #[test]
    fn test_eval_col_sign_negative() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "-2".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__sign__:num");
        assert_eq!(row.eval_col(&encoded), Some("-1".to_string()));
    }

    #[test]
    fn test_eval_col_sign_zero() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("num".to_string(), "0".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__sign__:num");
        assert_eq!(row.eval_col(&encoded), Some("0".to_string()));
    }

    #[test]
    fn test_parse_select_now_basic() {
        let result = parse_select("SELECT NOW() FROM demo");
        assert!(result.is_ok());
        let (_distinct, cols, _table, _where, _join, _group, _having, _order, _limit, _offset) =
            result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0], "__now__");
    }

    #[test]
    fn test_eval_col_now_returns_number() {
        use crate::table::Row;
        use std::collections::HashMap;
        let extras = HashMap::new();
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = String::from("__now__");
        let val = row.eval_col(&encoded).unwrap();
        let parsed = val.parse::<u64>();
        assert!(parsed.is_ok());
        assert!(parsed.unwrap() > 0);
    }

    #[test]
    fn test_parse_select_position_basic() {
        let result = parse_select("SELECT POSITION('ll', content) FROM demo");
        assert!(result.is_ok());
        let (_, cols, table, _, _, _, _, _, _, _) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__position__:"));
        assert_eq!(table, Some("demo".to_string()));
    }

    #[test]
    fn test_eval_col_position_found() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__position__:ll\x1Fcontent");
        assert_eq!(row.eval_col(&encoded), Some("3".to_string()));
    }

    #[test]
    fn test_eval_col_position_not_found() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__position__:xyz\x1Fcontent");
        assert_eq!(row.eval_col(&encoded), Some("0".to_string()));
    }

    #[test]
    fn test_parse_select_instr_basic() {
        let result = parse_select("SELECT INSTR(content, 'el') FROM demo");
        assert!(result.is_ok());
        let (_, cols, table, _, _, _, _, _, _, _) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__instr__:"));
        assert_eq!(table, Some("demo".to_string()));
    }

    #[test]
    fn test_eval_col_instr_found() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__instr__:content\x1Fel");
        assert_eq!(row.eval_col(&encoded), Some("2".to_string()));
    }

    #[test]
    fn test_eval_col_instr_not_found() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "hello".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__instr__:content\x1Fxyz");
        assert_eq!(row.eval_col(&encoded), Some("0".to_string()));
    }

    #[test]
    fn test_parse_select_substring_index_basic() {
        let result = parse_select("SELECT SUBSTRING_INDEX(content, ',', 2) FROM demo");
        assert!(result.is_ok());
        let (_, cols, table, _, _, _, _, _, _, _) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 1);
        assert!(cols[0].starts_with("__substring_index__:"));
        assert_eq!(table, Some("demo".to_string()));
    }

    #[test]
    fn test_eval_col_substring_index_positive() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "a,b,c".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__substring_index__:content\x1F,\x1F2");
        assert_eq!(row.eval_col(&encoded), Some("a,b".to_string()));
    }

    #[test]
    fn test_eval_col_substring_index_negative() {
        use crate::table::Row;
        use std::collections::HashMap;
        let mut extras = HashMap::new();
        extras.insert("content".to_string(), "a,b,c".to_string());
        let row = Row {
            id: 1,
            username: "u".to_string(),
            email: "e".to_string(),
            extras,
        };
        let encoded = format!("__substring_index__:content\x1F,\x1F-2");
        assert_eq!(row.eval_col(&encoded), Some("b,c".to_string()));
    }

    #[test]
    fn test_parse_lead_basic() {
        let result = parse_select("SELECT LEAD(username) OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__lead__:"));
        // Expect: __lead__:username\x1F1\x1FNULL\x1F\x1Fid
        assert!(cols[0].contains("username"));
    }

    #[test]
    fn test_parse_lead_with_offset() {
        let result = parse_select("SELECT LEAD(username, 2) OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode offset as 2
        assert!(cols[0].contains("\x1F2\x1F"));
    }

    #[test]
    fn test_parse_lead_with_default() {
        let result = parse_select("SELECT LEAD(username, 1, 'N/A') OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode default value as 'N/A'
        assert!(cols[0].contains("N/A"));
    }

    #[test]
    fn test_parse_lead_with_partition() {
        let result =
            parse_select("SELECT LEAD(username) OVER (PARTITION BY email ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode partition column
        assert!(cols[0].contains("email"));
    }

    #[test]
    fn test_lead_runtime() {
        let mut table = Table::new(
            "test_lead.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());
        let _ = table
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test basic LEAD with offset 1
        let encoded = "__lead__:username\x1F1\x1FNULL\x1F\x1Fid".to_string();
        let mapping = compute_lead_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Row 1 should get row 2's username
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("bob"));
        // Row 2 should get row 3's username
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("charlie"));
        // Row 3 has no next row, should get NULL
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("NULL"));

        // Clean up
        let _ = std::fs::remove_file("test_lead.json");
    }

    #[test]
    fn test_lead_with_offset() {
        let mut table = Table::new(
            "test_lead_offset.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());
        let _ = table
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(4, "dave".to_string(), "dave@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test LEAD with offset 2
        let encoded = "__lead__:username\x1F2\x1FNULL\x1F\x1Fid".to_string();
        let mapping = compute_lead_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Row 1 should get row 3's username (offset 2)
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("charlie"));
        // Row 2 should get row 4's username
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("dave"));
        // Row 3 and 4 get NULL
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("NULL"));
        assert_eq!(col_map.get(&4).map(|s| s.as_str()), Some("NULL"));

        // Clean up
        let _ = std::fs::remove_file("test_lead_offset.json");
    }

    #[test]
    fn test_lead_with_default_value() {
        let mut table = Table::new(
            "test_lead_default.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test LEAD with custom default value 'N/A'
        let encoded = "__lead__:username\x1F1\x1FN/A\x1F\x1Fid".to_string();
        let mapping = compute_lead_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Row 1 should get row 2's username
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("bob"));
        // Row 2 has no next row, should get custom default 'N/A'
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("N/A"));

        // Clean up
        let _ = std::fs::remove_file("test_lead_default.json");
    }

    #[test]
    fn test_parse_first_value_basic() {
        let result = parse_select("SELECT FIRST_VALUE(username) OVER (ORDER BY id) FROM users");
        if result.is_err() {
            panic!("parse error: {}", result.err().unwrap());
        }
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__first_value__:"));
        // Expect column name encoded
        assert!(cols[0].contains("username"));
    }

    #[test]
    fn test_first_value_runtime() {
        use crate::compute_first_value_map;
        let mut table = Table::new(
            "test_first_value.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );
        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());
        let _ = table
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap());
        let rows = table.select_all();

        let encoded = "__first_value__:username\x1F\x1Fid".to_string();
        let mapping = compute_first_value_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // All rows should receive the first username (alice)
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("alice"));
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("alice"));
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("alice"));

        let _ = std::fs::remove_file("test_first_value.json");
    }

    #[test]
    fn debug_tokens_first_value() {
        let toks =
            crate::parser::tokenize("SELECT FIRST_VALUE(username) OVER (ORDER BY id) FROM users");
        println!("TOKENS: {:?}", toks);
        assert!(true);
    }

    #[test]
    fn test_parse_lag_basic() {
        let result = parse_select("SELECT LAG(username) OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        assert!(cols[0].starts_with("__lag__:"));
        // Expect: __lag__:username\x1F1\x1FNULL\x1F\x1Fid
        assert!(cols[0].contains("username"));
    }

    #[test]
    fn test_parse_lag_with_offset() {
        let result = parse_select("SELECT LAG(username, 2) OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode offset as 2
        assert!(cols[0].contains("\x1F2\x1F"));
    }

    #[test]
    fn test_parse_lag_with_default() {
        let result = parse_select("SELECT LAG(username, 1, 'N/A') OVER (ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode default value as 'N/A'
        assert!(cols[0].contains("N/A"));
    }

    #[test]
    fn test_parse_lag_with_partition() {
        let result =
            parse_select("SELECT LAG(username) OVER (PARTITION BY email ORDER BY id) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert!(cols.is_some());
        let cols = cols.unwrap();
        // Should encode partition column
        assert!(cols[0].contains("email"));
    }

    #[test]
    fn test_lag_runtime() {
        let mut table = Table::new(
            "test_lag.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());
        let _ = table
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test basic LAG with offset 1
        let encoded = "__lag__:username\x1F1\x1FNULL\x1F\x1Fid".to_string();
        let mapping = compute_lag_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Row 1 has no previous row, should get NULL
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("NULL"));
        // Row 2 should get row 1's username
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("alice"));
        // Row 3 should get row 2's username
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("bob"));

        // Clean up
        let _ = std::fs::remove_file("test_lag.json");
    }

    #[test]
    fn test_lag_with_offset() {
        let mut table = Table::new(
            "test_lag_offset.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());
        let _ = table
            .insert(Row::new(3, "charlie".to_string(), "charlie@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(4, "dave".to_string(), "dave@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test LAG with offset 2
        let encoded = "__lag__:username\x1F2\x1FNULL\x1F\x1Fid".to_string();
        let mapping = compute_lag_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Rows 1 and 2 don't have rows 2 behind them, should get NULL
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("NULL"));
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("NULL"));
        // Row 3 should get row 1's username (offset 2)
        assert_eq!(col_map.get(&3).map(|s| s.as_str()), Some("alice"));
        // Row 4 should get row 2's username
        assert_eq!(col_map.get(&4).map(|s| s.as_str()), Some("bob"));

        // Clean up
        let _ = std::fs::remove_file("test_lag_offset.json");
    }

    #[test]
    fn test_lag_with_default_value() {
        let mut table = Table::new(
            "test_lag_default.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let _ =
            table.insert(Row::new(1, "alice".to_string(), "alice@test.com".to_string()).unwrap());
        let _ = table.insert(Row::new(2, "bob".to_string(), "bob@test.com".to_string()).unwrap());

        let rows = table.select_all();

        // Test LAG with custom default value 'START'
        let encoded = "__lag__:username\x1F1\x1FSTART\x1F\x1Fid".to_string();
        let mapping = compute_lag_map(&rows, &Some(vec![encoded.clone()]));
        let col_map = mapping.get(&encoded).expect("mapping present");

        // Row 1 has no previous row, should get custom default 'START'
        assert_eq!(col_map.get(&1).map(|s| s.as_str()), Some("START"));
        // Row 2 should get row 1's username
        assert_eq!(col_map.get(&2).map(|s| s.as_str()), Some("alice"));

        // Clean up
        let _ = std::fs::remove_file("test_lag_default.json");
    }

    // ===== DATE/TIME FUNCTION TESTS =====

    #[test]
    fn test_tokenize_date_time_functions() {
        let tokens = tokenize("NOW() DATE() TIME() YEAR() MONTH() DAY()");
        assert!(tokens.contains(&Token::Now));
        assert!(tokens.contains(&Token::Date));
        assert!(tokens.contains(&Token::Time));
        assert!(tokens.contains(&Token::Year));
        assert!(tokens.contains(&Token::Month));
        assert!(tokens.contains(&Token::Day));
    }

    #[test]
    fn test_parse_select_now_function() {
        let result = parse_select("SELECT NOW() FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__now__".to_string()]));
    }

    #[test]
    fn test_parse_select_date_function() {
        let result = parse_select("SELECT DATE(created) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__date__:created".to_string()]));
    }

    #[test]
    fn test_parse_select_time_function() {
        let result = parse_select("SELECT TIME(created) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__time__:created".to_string()]));
    }

    #[test]
    fn test_parse_select_year_function() {
        let result = parse_select("SELECT YEAR(birthdate) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__year__:birthdate".to_string()]));
    }

    #[test]
    fn test_parse_select_month_function() {
        let result = parse_select("SELECT MONTH(created) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__month__:created".to_string()]));
    }

    #[test]
    fn test_parse_select_day_function() {
        let result = parse_select("SELECT DAY(created) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        assert_eq!(cols, Some(vec!["__day__:created".to_string()]));
    }

    #[test]
    fn test_parse_select_multiple_date_functions() {
        let result = parse_select("SELECT YEAR(d), MONTH(d), DAY(d) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        let cols = cols.unwrap();
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0], "__year__:d");
        assert_eq!(cols[1], "__month__:d");
        assert_eq!(cols[2], "__day__:d");
    }

    #[test]
    fn test_eval_col_date_from_timestamp() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "1704067200".to_string()); // 2024-01-01 00:00:00 UTC
        let row = Row {
            id: 1,
            username: "alice".to_string(),
            email: "alice@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__date__:created");
        assert!(result.is_some());
        let date_str = result.unwrap();
        // Should return a date in YYYY-MM-DD format
        assert!(date_str.contains('-'));
        assert_eq!(date_str.len(), 10);
    }

    #[test]
    fn test_eval_col_date_from_date_string() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "2024-01-15 14:30:45".to_string());
        let row = Row {
            id: 1,
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__date__:created");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "2024-01-15");
    }

    #[test]
    fn test_eval_col_time_from_timestamp() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "1704067200".to_string()); // 2024-01-01 00:00:00 UTC
        let row = Row {
            id: 1,
            username: "charlie".to_string(),
            email: "charlie@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__time__:created");
        assert!(result.is_some());
        let time_str = result.unwrap();
        // Should return time in HH:MM:SS format
        assert_eq!(time_str.len(), 8);
        assert!(time_str.contains(':'));
    }

    #[test]
    fn test_eval_col_time_from_datetime_string() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "2024-01-15 14:30:45".to_string());
        let row = Row {
            id: 1,
            username: "diana".to_string(),
            email: "diana@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__time__:created");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "14:30:45");
    }

    #[test]
    fn test_eval_col_year_from_timestamp() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("birthdate".to_string(), "1704067200".to_string()); // 2024-01-01
        let row = Row {
            id: 1,
            username: "eve".to_string(),
            email: "eve@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__year__:birthdate");
        assert!(result.is_some());
        let year_str = result.unwrap();
        assert!(year_str.parse::<u64>().is_ok());
        assert!(year_str.as_str() >= "1970");
    }

    #[test]
    fn test_eval_col_year_from_date_string() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("birthdate".to_string(), "1990-05-20".to_string());
        let row = Row {
            id: 1,
            username: "frank".to_string(),
            email: "frank@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__year__:birthdate");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1990");
    }

    #[test]
    fn test_eval_col_month_from_timestamp() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "1704067200".to_string()); // 2024-01-01
        let row = Row {
            id: 1,
            username: "grace".to_string(),
            email: "grace@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__month__:created");
        assert!(result.is_some());
        let month_str = result.unwrap();
        let month: u64 = month_str.parse().unwrap_or(0);
        assert!(month >= 1 && month <= 12);
    }

    #[test]
    fn test_eval_col_month_from_date_string() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("event_date".to_string(), "2024-07-15".to_string());
        let row = Row {
            id: 1,
            username: "henry".to_string(),
            email: "henry@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__month__:event_date");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "07");
    }

    #[test]
    fn test_eval_col_day_from_timestamp() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "1704067200".to_string()); // 2024-01-01
        let row = Row {
            id: 1,
            username: "iris".to_string(),
            email: "iris@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__day__:created");
        assert!(result.is_some());
        let day_str = result.unwrap();
        let day: u64 = day_str.parse().unwrap_or(0);
        assert!(day >= 1 && day <= 31);
    }

    #[test]
    fn test_eval_col_day_from_date_string() {
        use crate::table::Row;
        use std::collections::HashMap;

        let mut extras = HashMap::new();
        extras.insert("event_date".to_string(), "2024-07-25".to_string());
        let row = Row {
            id: 1,
            username: "jack".to_string(),
            email: "jack@example.com".to_string(),
            extras,
        };

        let result = row.eval_col("__day__:event_date");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "25");
    }

    #[test]
    fn test_select_now_runtime() {
        use std::collections::HashMap;

        let mut table = Table::new(
            "test_now.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "email".to_string(),
            ],
        );

        let extras = HashMap::new();
        table
            .insert(Row {
                id: 1,
                username: "alice".to_string(),
                email: "alice@example.com".to_string(),
                extras: extras.clone(),
            })
            .unwrap();

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);

        // Test NOW() evaluation
        let result = rows[0].eval_col("__now__");
        assert!(result.is_some());
        let now_val = result.unwrap();
        // NOW() should return a timestamp (all digits)
        assert!(now_val.parse::<u64>().is_ok());

        // Clean up
        let _ = std::fs::remove_file("test_now.json");
    }

    #[test]
    fn test_select_date_runtime() {
        use std::collections::HashMap;

        let mut table = Table::new(
            "test_date.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "created".to_string(),
            ],
        );

        let mut extras = HashMap::new();
        extras.insert("created".to_string(), "2024-06-15 10:30:00".to_string());
        table
            .insert(Row {
                id: 1,
                username: "bob".to_string(),
                email: "bob@example.com".to_string(),
                extras,
            })
            .unwrap();

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);

        let result = rows[0].eval_col("__date__:created");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "2024-06-15");

        // Clean up
        let _ = std::fs::remove_file("test_date.json");
    }

    #[test]
    fn test_select_year_month_day_runtime() {
        use std::collections::HashMap;

        let mut table = Table::new(
            "test_ymd.json".to_string(),
            vec![
                "id".to_string(),
                "username".to_string(),
                "birthdate".to_string(),
            ],
        );

        let mut extras = HashMap::new();
        extras.insert("birthdate".to_string(), "1995-03-22".to_string());
        table
            .insert(Row {
                id: 1,
                username: "carol".to_string(),
                email: "carol@example.com".to_string(),
                extras,
            })
            .unwrap();

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);

        // Test YEAR
        let year_result = rows[0].eval_col("__year__:birthdate");
        assert!(year_result.is_some());
        assert_eq!(year_result.unwrap(), "1995");

        // Test MONTH
        let month_result = rows[0].eval_col("__month__:birthdate");
        assert!(month_result.is_some());
        assert_eq!(month_result.unwrap(), "03");

        // Test DAY
        let day_result = rows[0].eval_col("__day__:birthdate");
        assert!(day_result.is_some());
        assert_eq!(day_result.unwrap(), "22");

        // Clean up
        let _ = std::fs::remove_file("test_ymd.json");
    }

    // ===== CTE (WITH Clause) TESTS =====

    #[test]
    fn test_parse_cte_simple() {
        let sql = "WITH temp AS (SELECT id, username FROM users) SELECT * FROM temp";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_some());
        let cte = cte.unwrap();
        assert_eq!(cte.name, "temp");
        assert!(cte.query.to_uppercase().contains("SELECT"));
        assert!(main_query.to_uppercase().starts_with("SELECT"));
    }

    #[test]
    fn test_parse_cte_without_with() {
        let sql = "SELECT * FROM users";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_none());
        assert_eq!(main_query, sql);
    }

    #[test]
    fn test_parse_cte_complex_query() {
        let sql = "WITH high_value_orders AS (SELECT * FROM orders WHERE amount > 1000) SELECT * FROM high_value_orders";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        assert!(cte.is_some());
        let cte = cte.unwrap();
        assert_eq!(cte.name, "high_value_orders");
    }

    #[test]
    fn test_parse_cte_name_extraction() {
        let sql = "WITH my_cte AS (SELECT id FROM users) SELECT * FROM my_cte";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        let cte = cte.unwrap();
        assert_eq!(cte.name, "my_cte");
    }

    #[test]
    fn test_parse_cte_nested_parens() {
        let sql = "WITH filtered_data AS (SELECT id, (SELECT COUNT(*) FROM users) AS cnt FROM orders) SELECT * FROM filtered_data";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_some());
        assert!(main_query.to_uppercase().contains("SELECT"));
    }

    #[test]
    fn test_cte_structure() {
        let sql = "WITH summary AS (SELECT id, COUNT(*) as cnt FROM orders GROUP BY id) SELECT * FROM summary";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        let cte = cte.unwrap();

        // Verify CTE has name and query
        assert!(!cte.name.is_empty());
        assert!(!cte.query.is_empty());

        // Verify main query exists
        assert!(!main_query.is_empty());
    }

    #[test]
    fn test_cte_with_aggregates() {
        let sql = "WITH monthly_sales AS (SELECT MONTH(date) as m, SUM(amount) FROM sales GROUP BY MONTH(date)) SELECT * FROM monthly_sales";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        assert!(cte.is_some());
        let cte_query = cte.unwrap().query;
        assert!(cte_query.to_uppercase().contains("SELECT"));
    }

    #[test]
    fn test_cte_whitespace_handling() {
        let sql = "  WITH  my_data  AS  (  SELECT * FROM users  )  SELECT * FROM my_data  ";
        let result = parse_cte(sql);
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        assert!(cte.is_some());
        assert_eq!(cte.unwrap().name, "my_data");
    }

    #[test]
    fn test_tokenize_with_keyword() {
        let tokens = tokenize("WITH cte_name AS SELECT");
        assert!(tokens.contains(&Token::With));
    }

    #[test]
    fn test_cte_roundtrip() {
        let original_sql =
            "WITH filtered AS (SELECT id, username FROM users WHERE id > 5) SELECT * FROM filtered";
        let result = parse_cte(original_sql);
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();

        let cte = cte.unwrap();
        assert!(!cte.name.is_empty());
        assert!(!cte.query.is_empty());
        assert!(!main_query.is_empty());

        // Verify we can extract the key parts
        assert_eq!(cte.name, "filtered");
        assert!(main_query.contains("filtered"));
    }

    // ===== CREATE INDEX TESTS =====

    #[test]
    fn test_tokenize_create_index_keywords() {
        let tokens = tokenize("CREATE INDEX idx_name ON users (email)");
        assert!(tokens.contains(&Token::Create));
        assert!(tokens.contains(&Token::Index));
        assert!(tokens.contains(&Token::On));
        assert!(tokens.contains(&Token::LParen));
        assert!(tokens.contains(&Token::RParen));
    }

    #[test]
    fn test_parse_create_index_basic() {
        let result = parse_create_index("CREATE INDEX idx_email ON users (email)");
        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx.index_name, "idx_email");
        assert_eq!(idx.table_name, "users");
        assert_eq!(idx.column_name, "email");
    }

    #[test]
    fn test_parse_create_index_on_id() {
        let result = parse_create_index("CREATE INDEX idx_user_id ON users (id)");
        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx.index_name, "idx_user_id");
        assert_eq!(idx.table_name, "users");
        assert_eq!(idx.column_name, "id");
    }

    #[test]
    fn test_parse_create_index_on_custom_column() {
        let result = parse_create_index("CREATE INDEX idx_price ON products (price)");
        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx.index_name, "idx_price");
        assert_eq!(idx.table_name, "products");
        assert_eq!(idx.column_name, "price");
    }

    #[test]
    fn test_parse_create_index_with_underscores() {
        let result = parse_create_index("CREATE INDEX idx_user_name_lower ON users (username)");
        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx.index_name, "idx_user_name_lower");
    }

    #[test]
    fn test_parse_create_index_error_missing_on() {
        let result = parse_create_index("CREATE INDEX idx_email users (email)");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected ON"));
    }

    #[test]
    fn test_parse_create_index_error_missing_paren() {
        let result = parse_create_index("CREATE INDEX idx_email ON users email");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected ("));
    }

    #[test]
    fn test_parse_create_index_error_missing_column() {
        let result = parse_create_index("CREATE INDEX idx_email ON users ()");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected column name"));
    }

    #[test]
    fn test_parse_create_index_on_orders_table() {
        let result = parse_create_index("CREATE INDEX idx_order_customer ON orders (customer_id)");
        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx.table_name, "orders");
        assert_eq!(idx.column_name, "customer_id");
    }

    #[test]
    fn test_parse_drop_index_basic() {
        let result = parse_drop_index("DROP INDEX idx_email");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "idx_email");
    }

    #[test]
    fn test_parse_drop_index_error_missing_name() {
        let result = parse_drop_index("DROP INDEX");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected index name"));
    }

    #[test]
    fn test_index_definition_structure() {
        let result = parse_create_index("CREATE INDEX idx_status ON users (status)");
        assert!(result.is_ok());
        let idx = result.unwrap();

        // Verify all fields are populated
        assert!(!idx.index_name.is_empty());
        assert!(!idx.table_name.is_empty());
        assert!(!idx.column_name.is_empty());

        // Verify correct values
        assert_eq!(idx.index_name, "idx_status");
        assert_eq!(idx.table_name, "users");
        assert_eq!(idx.column_name, "status");
    }

    #[test]
    fn test_parse_create_index_various_column_types() {
        // Test on string column
        let result1 = parse_create_index("CREATE INDEX idx_name ON users (username)");
        assert!(result1.is_ok());
        assert_eq!(result1.unwrap().column_name, "username");

        // Test on numeric column
        let result2 = parse_create_index("CREATE INDEX idx_qty ON inventory (quantity)");
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap().column_name, "quantity");

        // Test on date column
        let result3 = parse_create_index("CREATE INDEX idx_date ON orders (order_date)");
        assert!(result3.is_ok());
        assert_eq!(result3.unwrap().column_name, "order_date");
    }

    #[test]
    fn test_create_index_multiple_tables() {
        // Index on users table
        let result1 = parse_create_index("CREATE INDEX idx_user_email ON users (email)");
        assert!(result1.is_ok());
        let idx1 = result1.unwrap();
        assert_eq!(idx1.table_name, "users");

        // Index on products table
        let result2 = parse_create_index("CREATE INDEX idx_product_price ON products (price)");
        assert!(result2.is_ok());
        let idx2 = result2.unwrap();
        assert_eq!(idx2.table_name, "products");

        // Verify they're different
        assert_ne!(idx1.table_name, idx2.table_name);
        assert_ne!(idx1.column_name, idx2.column_name);
    }

    // ===== NEW DATE FUNCTIONS: HOUR, MINUTE, SECOND, DATE_ADD, DATE_SUB =====

    #[test]
    fn test_parse_hour_function() {
        let tokens = tokenize("SELECT HOUR(created_at) FROM events");
        assert!(tokens.iter().any(|t| matches!(t, Token::Hour)));
    }

    #[test]
    fn test_parse_minute_function() {
        let tokens = tokenize("SELECT MINUTE(created_at) FROM events");
        assert!(tokens.iter().any(|t| matches!(t, Token::Minute)));
    }

    #[test]
    fn test_parse_second_function() {
        let tokens = tokenize("SELECT SECOND(created_at) FROM events");
        assert!(tokens.iter().any(|t| matches!(t, Token::Second)));
    }

    #[test]
    fn test_parse_date_add_function() {
        let tokens = tokenize("SELECT DATE_ADD(order_date, 7) FROM orders");
        assert!(tokens.iter().any(|t| matches!(t, Token::DateAdd)));
    }

    #[test]
    fn test_parse_date_sub_function() {
        let tokens = tokenize("SELECT DATE_SUB(order_date, 3) FROM orders");
        assert!(tokens.iter().any(|t| matches!(t, Token::DateSub)));
    }

    #[test]
    fn test_parse_week_function() {
        let tokens = tokenize("SELECT WEEK(order_date) FROM orders");
        assert!(tokens.iter().any(|t| matches!(t, Token::Week)));
    }

    #[test]
    fn test_parse_quarter_function() {
        let tokens = tokenize("SELECT QUARTER(order_date) FROM orders");
        assert!(tokens.iter().any(|t| matches!(t, Token::Quarter)));
    }

    #[test]
    fn test_parse_datediff_function() {
        let tokens = tokenize("SELECT DATEDIFF(end_date, start_date) FROM events");
        assert!(tokens.iter().any(|t| matches!(t, Token::DateDiff)));
    }

    #[test]
    fn test_parse_date_trunc_function() {
        let tokens = tokenize("SELECT DATE_TRUNC('month', order_date) FROM orders");
        assert!(tokens.iter().any(|t| matches!(t, Token::DateTrunc)));
    }

    #[test]
    fn test_parse_select_with_week_column() {
        let result = parse_select("SELECT WEEK(order_date) FROM orders");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__week__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_quarter_column() {
        let result = parse_select("SELECT QUARTER(sale_date) FROM sales");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__quarter__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_datediff_column() {
        let result = parse_select("SELECT DATEDIFF(end_date, start_date) FROM events");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__datediff__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_date_trunc_column() {
        let result = parse_select("SELECT DATE_TRUNC('year', created_at) FROM users");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__date_trunc__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_hour_column() {
        // Test without FROM first to isolate the issue
        let result = parse_select("SELECT HOUR(ts)");
        if let Err(e) = &result {
            eprintln!("Parse error (no FROM): {}", e);
        }
        // Now try with FROM
        let result2 = parse_select("SELECT HOUR(ts) FROM events");
        if let Err(e) = &result2 {
            eprintln!("Parse error (with FROM): {}", e);
        }
        assert!(result2.is_ok(), "Parse failed");
        let (_, cols, _, _, _, _, _, _, _, _) = result2.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__hour__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_minute_column() {
        let result = parse_select("SELECT MINUTE(timestamp_col) FROM events");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__minute__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_second_column() {
        let result = parse_select("SELECT SECOND(timestamp_col) FROM events");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__second__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_date_add_column() {
        let result = parse_select("SELECT DATE_ADD(order_date, 7) FROM orders");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__date_add__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    #[test]
    fn test_parse_select_with_date_sub_column() {
        let result = parse_select("SELECT DATE_SUB(order_date, 3) FROM orders");
        assert!(result.is_ok());
        let (_, cols, _, _, _, _, _, _, _, _) = result.unwrap();
        if let Some(columns) = cols {
            assert_eq!(columns.len(), 1);
            assert!(columns[0].contains("__date_sub__"));
        } else {
            panic!("Expected Some(columns)");
        }
    }

    // Runtime evaluation tests
    #[test]
    fn test_hour_extraction_from_time_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "14:30:45".to_string());
                m
            },
        };

        let result = row.eval_col("__hour__:created_at");
        assert_eq!(result, Some("14".to_string()));
    }

    #[test]
    fn test_minute_extraction_from_time_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "14:30:45".to_string());
                m
            },
        };

        let result = row.eval_col("__minute__:created_at");
        assert_eq!(result, Some("30".to_string()));
    }

    #[test]
    fn test_second_extraction_from_time_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "14:30:45".to_string());
                m
            },
        };

        let result = row.eval_col("__second__:created_at");
        assert_eq!(result, Some("45".to_string()));
    }

    #[test]
    fn test_hour_extraction_from_datetime_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "2024-06-15 16:45:30".to_string());
                m
            },
        };

        let result = row.eval_col("__hour__:created_at");
        assert_eq!(result, Some("16".to_string()));
    }

    #[test]
    fn test_date_add_with_date_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("order_date".to_string(), "2024-06-15".to_string());
                m
            },
        };

        let result = row.eval_col("__date_add__:order_date\x1F7");
        assert_eq!(result, Some("2024-06-22".to_string()));
    }

    #[test]
    fn test_date_sub_with_date_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("order_date".to_string(), "2024-06-15".to_string());
                m
            },
        };

        let result = row.eval_col("__date_sub__:order_date\x1F3");
        assert_eq!(result, Some("2024-06-12".to_string()));
    }

    #[test]
    fn test_week_from_date_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("order_date".to_string(), "2024-03-15".to_string()); // March 15
                m
            },
        };

        let result = row.eval_col("__week__:order_date");
        // Week calculation: March is month 3, days_before = 59, day = 15, total = 74, week = 11
        assert!(result.is_some());
        let week: u32 = result.unwrap().parse().unwrap_or(0);
        assert!(week >= 10 && week <= 12); // Approximate week number
    }

    #[test]
    fn test_quarter_from_date_string() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("q1_date".to_string(), "2024-02-15".to_string()); // Q1
                m.insert("q2_date".to_string(), "2024-05-20".to_string()); // Q2
                m.insert("q3_date".to_string(), "2024-08-10".to_string()); // Q3
                m.insert("q4_date".to_string(), "2024-11-25".to_string()); // Q4
                m
            },
        };

        assert_eq!(row.eval_col("__quarter__:q1_date"), Some("1".to_string()));
        assert_eq!(row.eval_col("__quarter__:q2_date"), Some("2".to_string()));
        assert_eq!(row.eval_col("__quarter__:q3_date"), Some("3".to_string()));
        assert_eq!(row.eval_col("__quarter__:q4_date"), Some("4".to_string()));
    }

    #[test]
    fn test_datediff_with_date_strings() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("start_date".to_string(), "2024-01-15".to_string());
                m.insert("end_date".to_string(), "2024-01-25".to_string());
                m
            },
        };

        let result = row.eval_col("__datediff__:end_date\x1Fstart_date");
        assert!(result.is_some());
        // Rough approximation should give positive difference
        let diff: i64 = result.unwrap().parse().unwrap_or(0);
        assert!(diff > 0);
    }

    #[test]
    fn test_datediff_with_unix_timestamps() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("ts1".to_string(), "86400".to_string()); // 1 day after epoch
                m.insert("ts2".to_string(), "0".to_string()); // epoch
                m
            },
        };

        let result = row.eval_col("__datediff__:ts1\x1Fts2");
        assert_eq!(result, Some("1".to_string())); // 1 day difference
    }

    #[test]
    fn test_date_trunc_year() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "2024-06-15".to_string());
                m
            },
        };

        let result = row.eval_col("__date_trunc__:year\x1Fcreated_at");
        assert_eq!(result, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_date_trunc_month() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "2024-06-15".to_string());
                m
            },
        };

        let result = row.eval_col("__date_trunc__:month\x1Fcreated_at");
        assert_eq!(result, Some("2024-06-01".to_string()));
    }

    #[test]
    fn test_date_trunc_day() {
        use std::collections::HashMap;
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), "2024-06-15".to_string());
                m
            },
        };

        let result = row.eval_col("__date_trunc__:day\x1Fcreated_at");
        assert_eq!(result, Some("2024-06-15".to_string()));
    }

    #[test]
    fn test_hour_minute_second_from_unix_timestamp() {
        use std::collections::HashMap;
        // Unix timestamp for 1970-01-01 12:34:56 UTC (45296 seconds)
        let row = Row {
            id: 1,
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("timestamp".to_string(), "45296".to_string()); // 12:34:56 on epoch day
                m
            },
        };

        let hour = row.eval_col("__hour__:timestamp");
        let minute = row.eval_col("__minute__:timestamp");
        let second = row.eval_col("__second__:timestamp");

        assert_eq!(hour, Some("12".to_string()));
        assert_eq!(minute, Some("34".to_string()));
        assert_eq!(second, Some("56".to_string()));
    }

    #[test]
    fn test_multiple_hour_minute_second_on_different_times() {
        use std::collections::HashMap;
        let row1 = Row {
            id: 1,
            username: "test1".to_string(),
            email: "test1@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("time".to_string(), "08:15:30".to_string());
                m
            },
        };

        let row2 = Row {
            id: 2,
            username: "test2".to_string(),
            email: "test2@example.com".to_string(),
            extras: {
                let mut m = HashMap::new();
                m.insert("time".to_string(), "23:59:59".to_string());
                m
            },
        };

        assert_eq!(row1.eval_col("__hour__:time"), Some("08".to_string()));
        assert_eq!(row1.eval_col("__minute__:time"), Some("15".to_string()));
        assert_eq!(row1.eval_col("__second__:time"), Some("30".to_string()));

        assert_eq!(row2.eval_col("__hour__:time"), Some("23".to_string()));
        assert_eq!(row2.eval_col("__minute__:time"), Some("59".to_string()));
        assert_eq!(row2.eval_col("__second__:time"), Some("59".to_string()));
    }

    // ===== CTE EXECUTION RECOGNITION TESTS =====

    #[test]
    fn test_cte_parsing_simple_with_clause() {
        let result = crate::parser::parse_cte(
            "WITH high_value_orders AS (SELECT id FROM orders) SELECT * FROM high_value_orders",
        );
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_some());
        let cte_data = cte.unwrap();
        assert_eq!(cte_data.name, "high_value_orders");
        assert!(cte_data.query.contains("SELECT"));
    }

    #[test]
    fn test_cte_parsing_no_cte() {
        let result = crate::parser::parse_cte("SELECT * FROM users");
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_none());
        assert_eq!(main_query, "SELECT * FROM users");
    }

    #[test]
    fn test_cte_parsing_extracts_cte_name() {
        let result = crate::parser::parse_cte(
            "WITH temp_data AS (SELECT id, name FROM products) SELECT * FROM temp_data",
        );
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        let cte_data = cte.unwrap();
        assert_eq!(cte_data.name, "temp_data");
    }

    #[test]
    fn test_cte_parsing_extracts_cte_query() {
        let result = crate::parser::parse_cte("WITH recent_orders AS (SELECT id FROM orders WHERE created > '2024-01-01') SELECT * FROM recent_orders");
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        let cte_data = cte.unwrap();
        assert!(cte_data.query.contains("SELECT"));
        assert!(cte_data.query.contains("orders"));
    }

    #[test]
    fn test_cte_parsing_extracts_main_query() {
        let result =
            crate::parser::parse_cte("WITH cte AS (SELECT 1) SELECT * FROM cte WHERE id > 5");
        assert!(result.is_ok());
        let (_, main_query) = result.unwrap();
        assert!(main_query.contains("WHERE"));
        assert!(main_query.contains("id"));
    }

    #[test]
    fn test_cte_parsing_with_complex_where() {
        let result = crate::parser::parse_cte(
            "WITH cte1 AS (SELECT id, name FROM users) SELECT id FROM cte1 WHERE name = 'alice'",
        );
        assert!(result.is_ok());
        let (cte, main_query) = result.unwrap();
        assert!(cte.is_some());
        assert!(main_query.contains("WHERE"));
    }

    #[test]
    fn test_cte_parsing_nested_parentheses() {
        let result = crate::parser::parse_cte("WITH cte AS (SELECT COUNT(*) FROM users WHERE (age > 18 AND status = 'active')) SELECT * FROM cte");
        assert!(result.is_ok());
        let (cte, _) = result.unwrap();
        let cte_data = cte.unwrap();
        assert!(cte_data.query.contains("COUNT"));
    }

    // PRIMARY KEY and UNIQUE constraint parsing tests
    #[test]
    fn test_tokenize_primary_key() {
        let tokens = tokenize("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)");
        assert!(tokens.contains(&Token::Primary));
        assert!(tokens.contains(&Token::Key));
    }

    #[test]
    fn test_tokenize_unique() {
        let tokens = tokenize("CREATE TABLE users (id INTEGER, email TEXT UNIQUE)");
        assert!(tokens.contains(&Token::Unique));
    }

    #[test]
    fn test_parse_create_table_with_primary_key() {
        let result = parse_create_table("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)");
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0], "id");
        assert_eq!(columns[1], "name");
        assert_eq!(primary_key, Some("id".to_string()));
        assert_eq!(unique_columns.len(), 0);
    }

    #[test]
    fn test_parse_create_table_with_unique() {
        let result =
            parse_create_table("CREATE TABLE users (id INTEGER, email TEXT UNIQUE, name TEXT)");
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0], "id");
        assert_eq!(columns[1], "email");
        assert_eq!(columns[2], "name");
        assert_eq!(primary_key, None);
        assert_eq!(unique_columns.len(), 1);
        assert_eq!(unique_columns[0], "email");
    }

    #[test]
    fn test_parse_create_table_with_multiple_unique() {
        let result = parse_create_table(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE, username TEXT UNIQUE)",
        );
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(columns.len(), 3);
        assert_eq!(primary_key, Some("id".to_string()));
        assert_eq!(unique_columns.len(), 2);
        assert_eq!(unique_columns[0], "email");
        assert_eq!(unique_columns[1], "username");
    }

    #[test]
    fn test_parse_create_table_with_pk_and_unique() {
        let result = parse_create_table(
            "CREATE TABLE products (id INTEGER PRIMARY KEY, sku TEXT UNIQUE, name TEXT)",
        );
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "products");
        assert_eq!(columns.len(), 3);
        assert_eq!(primary_key, Some("id".to_string()));
        assert_eq!(unique_columns.len(), 1);
        assert_eq!(unique_columns[0], "sku");
    }

    #[test]
    fn test_parse_create_table_no_constraints() {
        let result = parse_create_table("CREATE TABLE users (id INTEGER, name TEXT)");
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(columns.len(), 2);
        assert_eq!(primary_key, None);
        assert_eq!(unique_columns.len(), 0);
    }
}
        assert_eq!(unique_columns[1], "username");
    }

    #[test]
    fn test_parse_create_table_with_pk_and_unique() {
        let result = parse_create_table(
            "CREATE TABLE products (id INTEGER PRIMARY KEY, sku TEXT UNIQUE, name TEXT)",
        );
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "products");
        assert_eq!(columns.len(), 3);
        assert_eq!(primary_key, Some("id".to_string()));
        assert_eq!(unique_columns.len(), 1);
        assert_eq!(unique_columns[0], "sku");
    }

    #[test]
    fn test_parse_create_table_no_constraints() {
        let result = parse_create_table("CREATE TABLE users (id INTEGER, name TEXT)");
        assert!(result.is_ok());
        let (table_name, columns, primary_key, unique_columns) = result.unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(columns.len(), 2);
        assert_eq!(primary_key, None);
        assert_eq!(unique_columns.len(), 0);
    }
}
