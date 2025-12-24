use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};

const PAGE_SIZE: usize = 10; // Number of rows per page

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub rows: Vec<super::table::Row>,
}

impl Page {
    pub fn new() -> Self {
        Page { rows: Vec::new() }
    }

    pub fn is_full(&self) -> bool {
        self.rows.len() >= PAGE_SIZE
    }

    pub fn add_row(&mut self, row: super::table::Row) {
        self.rows.push(row);
    }
}

#[derive(Debug)]
pub struct Pager {
    pub pages: Vec<Page>,
    pub file_path: String,
    pub dirty: bool, // Track if changes need saving
}

impl Pager {
    pub fn new(file_path: String) -> Self {
        let pages = if std::path::Path::new(&file_path).exists() {
            let mut file = File::open(&file_path).unwrap();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();
            deserialize(&buffer).unwrap_or(Vec::new())
        } else {
            Vec::new()
        };
        Pager {
            pages,
            file_path,
            dirty: false,
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.dirty {
            let serialized = serialize(&self.pages)?;
            let mut file = File::create(&self.file_path)?;
            file.write_all(&serialized)?;
        }
        Ok(())
    }

    pub fn add_row(&mut self, row: super::table::Row) {
        if self.pages.is_empty() || self.pages.last().unwrap().is_full() {
            self.pages.push(Page::new());
        }
        self.pages.last_mut().unwrap().add_row(row);
        self.dirty = true;
    }

    pub fn get_page(&self, page_index: usize) -> Option<&Page> {
        self.pages.get(page_index)
    }

    pub fn get_page_mut(&mut self, page_index: usize) -> Option<&mut Page> {
        self.dirty = true;
        self.pages.get_mut(page_index)
    }

    pub fn get_all_rows(&self) -> Vec<&super::table::Row> {
        self.pages.iter().flat_map(|p| &p.rows).collect()
    }
}
