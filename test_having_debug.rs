use std::env;
use std::path::Path;

fn main() {
    // Add the parent directory to the module search path
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_path = Path::new(&manifest_dir).join("src");

    // We need to include the parser module
    println!("Testing HAVING clause parsing...");

    let test_sql = "SELECT username, COUNT(*) FROM users GROUP BY username HAVING count(*) > 2";
    println!("Parsing: {}", test_sql);

    // This would need the actual parser code
    println!("Debug: Check if 'count(*)' in HAVING needs to be parsed as an identifier");
}
