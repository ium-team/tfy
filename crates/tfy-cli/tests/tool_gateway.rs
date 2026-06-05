use std::process::Command;

#[test]
fn tool_gateway_summarizes_success_with_raw_ref() {
    let raw_dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            raw_dir.path().to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf ok",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let summary = String::from_utf8_lossy(&output.stdout);
    assert!(summary.contains("SUCCESS"), "{summary}");
    assert!(summary.contains("raw_ref="), "{summary}");
}

#[test]
fn tool_gateway_redacts_public_credential_urls() {
    let raw_dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            raw_dir.path().to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf 'https://user:secret@github.com/org/repo?token=secret\\n'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let summary = String::from_utf8_lossy(&output.stdout);
    assert!(summary.contains("https://github.com/org/repo"), "{summary}");
    assert!(!summary.contains("user:secret"), "{summary}");
    assert!(!summary.contains("token=secret"), "{summary}");
    assert!(summary.contains("raw_ref="), "{summary}");
}

#[test]
fn tool_gateway_preserves_failure_exit_code() {
    let raw_dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            raw_dir.path().to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf 'src/main.rs:1: error: broken\\n'; exit 7",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    let summary = String::from_utf8_lossy(&output.stdout);
    assert!(summary.contains("CRITICAL"), "{summary}");
    assert!(summary.contains("src/main.rs:1"), "{summary}");
    assert!(summary.contains("raw_ref="), "{summary}");
}
