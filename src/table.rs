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
            return Err(format!(
                "Username too long (max {} chars)",
                COLUMN_USERNAME_SIZE
            ));
        }
        if email.len() > COLUMN_EMAIL_SIZE {
            return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
        }
        Ok(Row {
            id,
            username,
            email,
        })
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

    /// Update a row by id.
    /// Returns an error if the id doesn't exist or the value is invalid for the column.
    pub fn update(&mut self, id: u32, column: &str, value: &str) -> Result<(), String> {
        if let Some(row) = self.rows.iter_mut().find(|r| r.id == id) {
            match column {
                "username" => {
                    if value.len() > COLUMN_USERNAME_SIZE {
                        return Err(format!(
                            "Username too long (max {} chars)",
                            COLUMN_USERNAME_SIZE
                        ));
                    }
                    row.username = value.to_string();
                }
                "email" => {
                    if value.len() > COLUMN_EMAIL_SIZE {
                        return Err(format!("Email too long (max {} chars)", COLUMN_EMAIL_SIZE));
                    }
                    row.email = value.to_string();
                }
                "id" => return Err("Cannot update id".to_string()),
                _ => return Err(format!("Unknown column '{}'", column)),
            }
            Ok(())
        } else {
            Err(format!("Row with id {} not found", id))
        }
    }
    pub fn delete(&mut self, id: u32) -> Result<(), String> {
        let initial_len = self.rows.len();
        self.rows.retain(|row| row.id != id);

        if self.rows.len() == initial_len {
            Err(format!("Row with id {} not found", id))
        } else {
            Ok(())
        }
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

    #[test]
    fn update_modifies_existing_row() {
        let mut table = Table::new();

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        table.insert(r1).unwrap();

        // Update username
        assert!(table.update(1, "username", "alice2").is_ok());
        let rows = table.select_all();
        assert_eq!(rows[0].username, "alice2");

        // Update email
        assert!(table.update(1, "email", "alice2@example.com").is_ok());
        let rows = table.select_all();
        assert_eq!(rows[0].email, "alice2@example.com");

        // Update non-existent id
        assert!(table.update(2, "username", "bob").is_err());

        // Update invalid column
        assert!(table.update(1, "invalid", "value").is_err());

        // Update id (should fail)
        assert!(table.update(1, "id", "2").is_err());
    }
}
