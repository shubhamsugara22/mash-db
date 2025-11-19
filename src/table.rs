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

    pub fn insert(&mut self, row: Row) {
        self.rows.push(row);
    }

    pub fn select_all(&self) -> &[Row] {
        &self.rows
    }
}
