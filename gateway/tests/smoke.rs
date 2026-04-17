use std::process::Command;

#[test]
fn smoke_help_command_exits_successfully() {
    let binary = env!("CARGO_BIN_EXE_gateway");
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .expect("failed to run gateway --help");

    assert!(
        output.status.success(),
        "gateway --help should exit 0\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Usage:") || stderr.contains("Usage:"),
        "help output should contain usage text"
    );
}
