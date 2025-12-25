use std::io::{self, Write};

mod column;
mod pager;
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
    SelectWhere {
        columns: Option<Vec<String>>,
        column: String,
        value: String,
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

fn do_meta_command(input: &str, _table: &mut Table) -> MetaCommandResult {
    if input == ".exit" {
        println!("Bye!");
        std::process::exit(0);
    } else {
        MetaCommandResult::UnrecognizedCommand
    }
}

fn prepare_statement(input: &str) -> PrepareResult {
    if input.to_uppercase().starts_with("INSERT") {
        match parser::parse_insert(input) {
            Ok((id, username, email)) => PrepareResult::Success(Statement::Insert {
                id,
                username,
                email,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase().starts_with("UPDATE") {
        match parser::parse_update(input) {
            Ok((id, column, value)) => {
                PrepareResult::Success(Statement::Update { id, column, value })
            }
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase().starts_with("SELECT") {
        match parser::parse_select(input) {
            Ok((cols, None)) => PrepareResult::Success(Statement::Select { columns: cols }),
            Ok((cols, Some((col, val)))) => PrepareResult::Success(Statement::SelectWhere {
                columns: cols,
                column: col,
                value: val,
            }),
            Err(_) => PrepareResult::UnrecognizedStatement,
        }
    } else if input.to_uppercase() == "DELETE ALL" {
        PrepareResult::Success(Statement::DeleteAll)
    } else if input.to_uppercase().starts_with("DELETE") {
        if input.to_uppercase().contains("WHERE") {
            match parser::parse_delete_where(input) {
                Ok((column, value)) => {
                    PrepareResult::Success(Statement::DeleteWhere { column, value })
                }
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        } else {
            match parser::parse_delete(input) {
                Ok(id) => PrepareResult::Success(Statement::Delete { id }),
                Err(_) => PrepareResult::UnrecognizedStatement,
            }
        }
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
                Ok(()) => {
                    table.save().unwrap();
                    println!("Executed.");
                }
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
        Statement::SelectWhere {
            columns,
            column,
            value,
        } => match table.select_where(&column, &value) {
            Ok(rows) => {
                for row in rows {
                    match &columns {
                        None => println!("({}, {}, {})", row.id, row.username, row.email),
                        Some(cols) => {
                            let mut values = Vec::new();
                            for col in cols {
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
            Err(e) => println!("Error: {}", e),
        },
        Statement::Update { id, column, value } => match table.update(id, &column, &value) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::Delete { id } => match table.delete(id) {
            Ok(()) => {
                table.save().unwrap();
                println!("Executed.");
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::DeleteWhere { column, value } => match table.delete_where(&column, &value) {
            Ok(count) => {
                table.save().unwrap();
                println!("Deleted {} rows.", count);
            }
            Err(e) => println!("Error: {}", e),
        },
        Statement::DeleteAll => {
            let count = table.clear();
            table.save().unwrap();
            println!("Deleted {} rows.", count);
        }
    }
}

fn main() {
    let mut table = Table::new("data.json".to_string());

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
            match do_meta_command(input, &mut table) {
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
