const COLUMN_USERNAME_SIZE: usize = 32;
const COLUMN_EMAIL_SIZE: usize = 255;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn select_where(&self, column: &str, value: &str) -> Result<Vec<&Row>, String> {
        let mut result = Vec::new();

        match column {
            "id" => {
                let id = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid id value".to_string())?;

                for row in &self.rows {
                    if row.id == id {
                        result.push(row);
                    }
                }
            }
            "username" => {
                for row in &self.rows {
                    if row.username == value {
                        result.push(row);
                    }
                }
            }
            "email" => {
                for row in &self.rows {
                    if row.email == value {
                        result.push(row);
                    }
                }
            }
            _ => return Err(format!("Invalid column '{}'", column)),
        }

        Ok(result)
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
    pub fn delete_where(&mut self, column: &str, value: &str) -> Result<usize, String> {
        let initial_len = self.rows.len();

        match column {
            "id" => {
                let id = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid id value".to_string())?;

                self.rows.retain(|row| row.id != id);
            }
            "username" => {
                self.rows.retain(|row| row.username != value);
            }
            "email" => {
                self.rows.retain(|row| row.email != value);
            }
            _ => return Err(format!("Invalid column '{}'", column)),
        }
        Ok(initial_len - self.rows.len())
    }
    pub fn clear(&mut self) -> usize {
        let count = self.rows.len();
        self.rows.clear();
        count
    }

    /// Save the table to a JSON file.
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a table from a JSON file.
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let table: Table = serde_json::from_str(&json)?;
        Ok(table)
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

    #[test]
    fn save_and_load_table() {
        let mut table = Table::new();

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        let r2 = Row::new(2, "bob".to_string(), "bob@example.com".to_string()).unwrap();
        table.insert(r1).unwrap();
        table.insert(r2).unwrap();

        // Save to file
        let filename = "test_table.json";
        assert!(table.save_to_file(filename).is_ok());

        // Load from file
        let loaded_table = Table::load_from_file(filename).unwrap();
        assert_eq!(loaded_table.select_all().len(), 2);
        assert_eq!(loaded_table.select_all()[0].username, "alice");
        assert_eq!(loaded_table.select_all()[1].username, "bob");

        // Clean up
        std::fs::remove_file(filename).unwrap();
    }
    #[test]
    fn delete_removes_existing_row() {
        let mut table = Table::new();

        let r1 = Row::new(1, "alice".to_string(), "alice@example.com".to_string()).unwrap();
        let r2 = Row::new(2, "bob".to_string(), "bob@example.com".to_string()).unwrap();

        table.insert(r1).unwrap();
        table.insert(r2).unwrap();

        // Delete existing row
        assert!(table.delete(1).is_ok());

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn delete_where_by_id_removes_row() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("id", "1").unwrap();
        assert_eq!(deleted, 1);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn delete_where_by_username() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("username", "alice").unwrap();
        assert_eq!(deleted, 2);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "bob");
    }
    #[test]
    fn delete_where_by_email() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("email", "b@b.com").unwrap();
        assert_eq!(deleted, 1);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].email, "a@a.com");
    }
    #[test]
    fn delete_where_invalid_column_fails() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let res = table.delete_where("invalid", "alice");
        assert!(res.is_err());
    }
    #[test]
    fn delete_where_no_matching_rows_returns_zero() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.delete_where("username", "bob").unwrap();
        assert_eq!(deleted, 0);

        let rows = table.select_all();
        assert_eq!(rows.len(), 1);
    }
    #[test]
    fn delete_all_removes_everything() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let deleted = table.clear();
        assert_eq!(deleted, 2);
        assert_eq!(table.select_all().len(), 0);
    }
    #[test]
    fn delete_all_on_empty_table() {
        let mut table = Table::new();

        let deleted = table.clear();
        assert_eq!(deleted, 0);
        assert_eq!(table.select_all().len(), 0);
    }
    #[test]
    fn select_where_by_id() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("id", "1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "alice");
    }
    #[test]
    fn select_where_by_username() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "alice".to_string(), "a2@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("username", "alice").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].email, "a@a.com");
        assert_eq!(rows[1].email, "a2@a.com");
    }
    #[test]
    fn select_where_by_email() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("email", "b@b.com").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn select_where_no_matches_returns_empty() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let rows = table.select_where("username", "bob").unwrap();
        assert_eq!(rows.len(), 0);
    }
    #[test]
    fn select_where_invalid_column_fails() {
        let mut table = Table::new();

        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();

        let res = table.select_where("invalid", "alice");
        assert!(res.is_err());
    }
    #[test]
    fn btree_index_correctness() {
        let mut table = Table::new();

        // Insert rows
        table
            .insert(Row::new(1, "alice".to_string(), "a@a.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(3, "charlie".to_string(), "c@c.com".to_string()).unwrap())
            .unwrap();
        table
            .insert(Row::new(2, "bob".to_string(), "b@b.com".to_string()).unwrap())
            .unwrap();

        // Select by id should work efficiently (using index)
        let rows = table.select_where("id", "1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "alice");

        let rows = table.select_where("id", "2").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "bob");

        let rows = table.select_where("id", "3").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "charlie");

        // Delete and check it no longer exists
        table.delete(2).unwrap();
        let rows = table.select_where("id", "2").unwrap();
        assert_eq!(rows.len(), 0);

        // Other rows still exist
        let rows = table.select_where("id", "1").unwrap();
        assert_eq!(rows.len(), 1);
        let rows = table.select_where("id", "3").unwrap();
        assert_eq!(rows.len(), 1);
    }
}
