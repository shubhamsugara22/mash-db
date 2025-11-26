const COLUMN_USERNAME_SIZE: usize = 32;
const COLUMN_EMAIL_SIZE: usize = 255;

#[derive(Debug, Clone)]
pub struct Row {
    pub id: u32,
    pub username: String,
    pub email: String,
}

impl Row {
    pub fn new(id: u32, username: String, email: String) -> Result<Self, String> {
        if username.len() > COLUMN_USERNAME_SIZE {
            return Err(format!("Username too long (max {} chars)", COLUMN_USERNAME_SIZE));
        }
        if email.len() > COLUMN_EMAIL_SIZE {
            return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
        }
        Ok(Row { id, username, email })
    }
}

pub struct Table {
    rows: Vec<Row>,
}

impl Table {
    pub fn new() -> Self {
        Table { rows: Vec::new() }
    }

    /// Insert a row into the table.
    /// Returns an error if a row with the same `id` already exists.
    pub fn insert(&mut self, row: Row) -> Result<(), String> {
        if self.rows.iter().any(|r| r.id == row.id) {
            return Err(format!("Duplicate id {}", row.id));
        }
        self.rows.push(row);
        Ok(())
    }

    pub fn select_all(&self) -> &[Row] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_validation_rejects_long_username_and_email() {
        let long_username = "a".repeat(COLUMN_USERNAME_SIZE + 1);
        let long_email = "b".repeat(COLUMN_EMAIL_SIZE + 1);

        assert!(Row::new(1, long_username.clone(), "ok@example.com".to_string()).is_err());
        assert!(Row::new(1, "ok".to_string(), long_email.clone()).is_err());
    }

    #[test]
    fn insert_prevents_duplicate_id() {
        let mut table = Table::new();

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        assert!(table.insert(r1).is_ok());

        let r2 = Row::new(1, "bob".to_string(), "bob@example.com".to_string()).unwrap();
        let res = table.insert(r2);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Duplicate id 1");
    }
}
