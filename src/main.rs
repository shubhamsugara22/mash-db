use std::io::{self, Write};

mod column;
mod parser;
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
    Insert {
        id: u32,
        username: String,
        email: String,
    },
    Select {
        columns: Option<Vec<String>>,
    },
    Update {
        id: u32,
        column: String,
        value: String,
    },
    Delete {
        id: u32,
    },
    DeleteWhere {
        column: String,
        value: String,
    },
    DeleteAll,
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

        PrepareResult::Success(Statement::Insert {
            id,
            username,
            email,
        })
    } else if input.to_lowercase().starts_with("update") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 5 || parts[2].to_lowercase() != "set" {
            return PrepareResult::UnrecognizedStatement;
        }

        let id = match parts[1].parse::<u32>() {
            Ok(id) => id,
            Err(_) => return PrepareResult::UnrecognizedStatement,
        };

        let set_part = parts[3];
        let eq_pos = set_part.find('=');
        if eq_pos.is_none() {
            return PrepareResult::UnrecognizedStatement;
        }
        let eq_pos = eq_pos.unwrap();
        let column = set_part[..eq_pos].to_string();
        let value = set_part[eq_pos + 1..].to_string();

        PrepareResult::Success(Statement::Update { id, column, value })
    } else if input.to_lowercase().starts_with("select") {
        match parser::parse_select(input) {
            Ok(cols) => PrepareResult::Success(Statement::Select { columns: cols }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_lowercase().starts_with("delete") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 2 {
            return PrepareResult::UnrecognizedStatement;
        }

        let id = match parts[1].parse::<u32>() {
            Ok(id) => id,
            Err(_) => return PrepareResult::UnrecognizedStatement,
        };
        PrepareResult::Success(Statement::Delete { id })

    } else if input.to_lowercase().starts_with("delete where") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 3 {
            return PrepareResult::UnrecognizedStatement;
        }

        let cond = parts[2];
        let eq_pos = cond.find('=');
        if eq_pos.is_none() {
            return PrepareResult::UnrecognizedStatement;
        }

        let eq_pos = eq_pos.unwrap();
        let column = cond[..eq_pos].to_string();
        let value = cond[eq_pos + 1..].to_string();

        PrepareResult::Success(Statement::DeleteWhere { column, value })
    
    } else if input.to_lowercase() == "delete all" {
        PrepareResult::Success(Statement::DeleteAll)
    
    } else {
        PrepareResult::UnrecognizedStatement
    } 
}

fn execute_statement(statement: Statement, table: &mut Table) {
    match statement {
        Statement::Insert {
            id,
            username,
            email,
        } => match Row::new(id, username, email) {
            Ok(row) => match table.insert(row) {
                Ok(()) => println!("Executed."),
                Err(e) => println!("Error: {}", e),
            },
            Err(e) => println!("Error: {}", e),
        },
        Statement::Select { columns } => {
            for row in table.select_all() {
                match &columns {
                    None => println!("({}, {}, {})", row.id, row.username, row.email),
                    Some(cols) => {
                        let mut values: Vec<String> = Vec::new();
                        for col in cols.iter() {
                            match col.as_str() {
                                "id" => values.push(row.id.to_string()),
                                "username" => values.push(row.username.clone()),
                                "email" => values.push(row.email.clone()),
                                other => values.push(format!("NULL({})", other)),
                            }
                        }
                        println!("({})", values.join(", "));
                    }
                }
            }
            println!("Executed.");
        }
        Statement::Update { id, column, value } => match table.update(id, &column, &value) {
            Ok(()) => println!("Executed."),
            Err(e) => println!("Error: {}", e),
        }
        Statement::Delete { id } => match table.delete(id) {
            Ok(()) => println!("Executed."),
            Err(e) => println!("Error: {}", e),
        }
        Statement::DeleteWhere { column, value } => match table.delete_where(&column, &value) {
            Ok(count) => println!("Deleted {} rows.", count),
            Err(e) => println!("Error: {}", e),
        }
        Statement::DeleteAll => {
            let count = table.clear();
            println!("Deleted {} rows.", count);
        },
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
