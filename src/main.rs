use std::io::{self, Write};

mod column;
mod table;

enum MetaCommandResult {
    Success,
    UnrecognizedCommand,
}

enum PrepareResult {
    Success,
    UnrecognizedStatement,
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
        PrepareResult::Success
    } else if input.starts_with("select") {
        PrepareResult::Success
    } else {
        PrepareResult::UnrecognizedStatement
    }
}

fn main() {
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
            PrepareResult::Success => {
                println!("Executing: {}", input);
            }
            PrepareResult::UnrecognizedStatement => {
                println!("Unrecognized keyword at start of '{}'", input);
            }
        }
    }
}

