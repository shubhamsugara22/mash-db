use std::io::{self, Write};

mod column;
mod table;

use table::{Row, Table};

enum MetaCommandResult {
    Success,
    UnrecognizedCommand,
}

enum PrepareResult {
    Success(Statement),
    UnrecognizedStatement,
}

enum Statement {
    Insert { id: u32, username: String, email: String },
    Select,
}

fn print_prompt() {
    print!("db > ");
    io::stdout().flush().unwrap();
}

fn do_meta_command(input: &str) -> MetaCommandResult {
    match input {
        ".exit" => {
            println!("Bye!");
            std::process::exit(0);
        }
        _ => MetaCommandResult::UnrecognizedCommand,
    }
}

fn prepare_statement(input: &str) -> PrepareResult {
    if input.starts_with("insert") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 4 {
            return PrepareResult::UnrecognizedStatement;
        }
        
        let id = match parts[1].parse::<u32>() {
            Ok(id) => id,
            Err(_) => return PrepareResult::UnrecognizedStatement,
        };
        
        let username = parts[2].to_string();
        let email = parts[3].to_string();
        
        PrepareResult::Success(Statement::Insert { id, username, email })
    } else if input.starts_with("select") {
        PrepareResult::Success(Statement::Select)
    } else {
        PrepareResult::UnrecognizedStatement
    }
}

fn execute_statement(statement: Statement, table: &mut Table) {
    match statement {
        Statement::Insert { id, username, email } => {
            match Row::new(id, username, email) {
                Ok(row) => {
                    table.insert(row);
                    println!("Executed.");
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }
        Statement::Select => {
            for row in table.select_all() {
                println!("({}, {}, {})", row.id, row.username, row.email);
            }
            println!("Executed.");
        }
    }
}

fn main() {
    let mut table = Table::new();
    
    loop {
        print_prompt();
        
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input.starts_with('.') {
            match do_meta_command(input) {
                MetaCommandResult::Success => continue,
                MetaCommandResult::UnrecognizedCommand => {
                    println!("Unrecognized command '{}'", input);
                    continue;
                }
            }
        }
        
        match prepare_statement(input) {
            PrepareResult::Success(statement) => {
                execute_statement(statement, &mut table);
            }
            PrepareResult::UnrecognizedStatement => {
                println!("Unrecognized keyword at start of '{}'", input);
            }
        }
    }
}

