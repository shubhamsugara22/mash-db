use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn make_temp_dir() -> PathBuf {
    let unique = format!(
        "mash_db_it_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    );
    let dir = env::temp_dir().join(unique);
    fs::create_dir_all(&dir).expect("failed to create temporary test directory");
    dir
}

#[test]
fn test_repl_numeric_literals_script() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir
        .join("Test-commands")
        .join("test_numeric_literals.txt");
    let script = fs::read_to_string(&script_path)
        .unwrap_or_else(|e| panic!("failed to read script {}: {}", script_path.display(), e));

    let temp_dir = make_temp_dir();

    let mut child = Command::new(env!("CARGO_BIN_EXE_Mash_db"))
        .current_dir(&temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start Mash_db binary");

    child
        .stdin
        .as_mut()
        .expect("child stdin should be available")
        .write_all(script.as_bytes())
        .expect("failed to pipe script into Mash_db stdin");

    let output = child
        .wait_with_output()
        .expect("failed to wait for Mash_db process");

    let _ = fs::remove_dir_all(&temp_dir);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Mash_db exited unsuccessfully. stderr:\n{}\nstdout:\n{}",
        stderr,
        stdout
    );

    assert!(
        stdout.contains("(1, baseline, 19.99, normal)"),
        "expected baseline row in output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("(spike, 1.25e3)"),
        "expected scientific notation select output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("(half, .5)"),
        "expected leading-dot select output, got:\n{}",
        stdout
    );
}
