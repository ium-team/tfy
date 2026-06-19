use sha2::{Digest, Sha256};
use std::process::Command;

struct IsolatedToolPaths {
    _tmp: tempfile::TempDir,
    raw_dir: String,
    ledger: String,
}

fn isolated_tool_paths() -> IsolatedToolPaths {
    let tmp = tempfile::tempdir().unwrap();
    IsolatedToolPaths {
        raw_dir: tmp.path().join("raw").to_str().unwrap().to_string(),
        ledger: tmp
            .path()
            .join("ledger.jsonl")
            .to_str()
            .unwrap()
            .to_string(),
        _tmp: tmp,
    }
}

#[test]
fn tool_gateway_passes_through_tiny_success_without_json_or_ref_overhead() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf ok",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(model_text, "ok");
    assert!(!model_text.trim_start().starts_with('{'), "{model_text}");
    assert!(!model_text.contains("raw_ref="), "{model_text}");
}

#[test]
fn tool_gateway_redacts_public_credential_urls() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf 'https://user:secret@github.com/org/repo?token=secret\\n'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(
        model_text.contains("https://github.com/org/repo"),
        "{model_text}"
    );
    assert!(!model_text.contains("user:secret"), "{model_text}");
    assert!(!model_text.contains("token=secret"), "{model_text}");
    assert!(!model_text.trim_start().starts_with('{'), "{model_text}");
    assert!(!model_text.contains("raw_ref="), "{model_text}");
}

#[test]
fn tool_gateway_preserves_failure_exit_code() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf 'src/main.rs:1: error: broken\\n'; exit 7",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("src/main.rs:1"), "{model_text}");
    assert!(model_text.contains("error: broken"), "{model_text}");
    assert!(!model_text.trim_start().starts_with('{'), "{model_text}");
    assert!(!model_text.contains("raw_ref="), "{model_text}");
}

#[test]
fn tool_gateway_summarizes_long_output_only_when_shorter() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 200); do echo \"line $i repeated build noise\"; done",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("TFY command summary"), "{model_text}");
    assert!(model_text.contains("raw_ref="), "{model_text}");
    assert!(model_text.len() < 200 * "line 000 repeated build noise\n".len());
}

#[test]
fn tool_gateway_suppresses_binary_with_recoverable_ref() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "python3",
            "-c",
            "import sys; sys.stdout.buffer.write(b'abc\\x00def')",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("output suppressed"), "{model_text}");
    assert!(model_text.contains("raw_ref="), "{model_text}");
}

#[test]
fn tool_gateway_raw_ref_recovers_invalid_utf8_bytes_losslessly() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "python3",
            "-c",
            "import sys; sys.stdout.buffer.write(b'abc\\xffdef')",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("output suppressed"), "{model_text}");
    let raw_ref = model_text
        .split("raw_ref=")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches(']')
        .to_string();
    let raw = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["raw", "--raw-dir", paths.raw_dir.as_str(), &raw_ref])
        .output()
        .unwrap();
    assert!(raw.status.success());
    assert_eq!(raw.stdout, b"abc\xffdef");
}

#[test]
fn raw_around_returns_only_requested_text_range() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf 'a\\nneedle\\nc\\nd\\n'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let raw_ref = json["payload"]["raw_ref"].as_str().unwrap().to_string();
    let raw = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "raw",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--around",
            "needle",
            "--context",
            "0",
            &raw_ref,
        ])
        .output()
        .unwrap();
    assert!(raw.status.success());
    assert_eq!(raw.stdout, b"needle\n");
}

#[test]
fn raw_around_invalid_utf8_fails_closed_without_lossy_output() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "python3",
            "-c",
            "import sys; sys.stdout.buffer.write(b'abc\\xffdef')",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let model_text = String::from_utf8_lossy(&output.stdout);
    let raw_ref = model_text
        .split("raw_ref=")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches(']')
        .to_string();
    let raw = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "raw",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--around",
            "abc",
            &raw_ref,
        ])
        .output()
        .unwrap();
    assert!(!raw.status.success());
    assert!(raw.stdout.is_empty());
}

#[test]
fn raw_around_missing_needle_fails_closed_without_full_output() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf 'a\\nneedle\\nc\\nd\\n'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let raw_ref = json["payload"]["raw_ref"].as_str().unwrap().to_string();
    let raw = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "raw",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--around",
            "absent",
            &raw_ref,
        ])
        .output()
        .unwrap();
    assert!(!raw.status.success());
    assert!(raw.stdout.is_empty());
}

#[test]
fn tool_gateway_json_includes_command_family_for_p0_wrapped_command() {
    let paths = isolated_tool_paths();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "cargo",
            "test",
            "--help",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["command_family"], "cargo_test");
    assert_eq!(json["payload"]["strategy_kind"], "rust");
    assert_eq!(json["payload"]["agent_safe"], true);
    assert_eq!(json["payload"]["interactive_risk"], "none");
    assert_eq!(json["payload"]["rendering_kind"], "summary");
    assert!(json["payload"]["model_text"]
        .as_str()
        .unwrap()
        .contains("family=cargo_test"));
}

#[test]
fn tool_gateway_uses_user_global_toml_rule_for_custom_command() {
    let paths = isolated_tool_paths();
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".config/tfy");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("commands.toml"),
        r#"
[[command]]
id = "custom_shell"
match.argv_prefix = ["sh", "-c"]
preserve_lines_matching = ["(?i)(error|warning)"]
strip_lines_matching = ["(?i)^noise"]
head_lines = 4
tail_lines = 2
max_lines = 8
truncate_lines_at = 160
human_auto_safe = true
agent_safe = true
interactive_risk = "none"
"#,
    )
    .unwrap();
    let script =
        "for i in $(seq 1 120); do echo noise-$i; done; echo 'ERROR token=super-secret-value'";
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            script,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["strategy_kind"], "user_toml");
    assert_eq!(json["payload"]["rule_id"], "custom_shell");
    assert_eq!(json["payload"]["strategy_source_kind"], "user");
    assert_eq!(json["payload"]["rendering_kind"], "summary");
    let model_text = json["payload"]["model_text"].as_str().unwrap();
    assert!(model_text.contains("strategy=user_toml"), "{model_text}");
    assert!(!model_text.contains("super-secret-value"), "{model_text}");
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "local-session",
            "--ledger",
            paths.ledger.as_str(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let report_json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report_json["strategy_counts"]["user_toml"], 1);
    assert_eq!(report_json["rule_counts"]["custom_shell"], 1);
    assert_eq!(report_json["strategy_source_counts"]["user"], 1);
}

#[test]
fn tool_gateway_skips_untrusted_repo_local_toml_rule() {
    let paths = isolated_tool_paths();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(work.path().join(".tfy")).unwrap();
    std::fs::write(
        work.path().join(".tfy/commands.toml"),
        r#"
[[command]]
id = "repo_rule"
match.argv_prefix = ["sh", "-c"]
preserve_lines_matching = ["ERROR"]
max_lines = 8
"#,
    )
    .unwrap();
    let nested = work.path().join("nested/child");
    std::fs::create_dir_all(&nested).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(&nested)
        .env("HOME", work.path().join("empty-home"))
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "printf 'ERROR should not use repo rule\\n'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("repo_rules_untrusted"), "{stderr}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(json["payload"]["strategy_kind"], "user_toml");
    assert_eq!(
        json["payload"]["command_rule_diagnostics"][0]["code"],
        "repo_rules_untrusted"
    );
}

#[test]
fn tool_gateway_applies_hash_trusted_repo_local_toml_rule() {
    let paths = isolated_tool_paths();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(work.path().join(".tfy")).unwrap();
    let rules_path = work.path().join(".tfy/commands.toml");
    let rules = r#"
[[command]]
id = "repo_rule"
match.argv_prefix = ["sh", "-c"]
preserve_lines_matching = ["ERROR"]
strip_lines_matching = ["noise"]
max_lines = 8
"#;
    std::fs::write(&rules_path, rules).unwrap();
    let hash = format!("{:x}", Sha256::digest(rules.as_bytes()));
    std::fs::write(
        work.path().join(".tfy/trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();
    let nested = work.path().join("nested/child");
    std::fs::create_dir_all(&nested).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(&nested)
        .env("HOME", work.path().join("empty-home"))
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            paths.raw_dir.as_str(),
            "--ledger",
            paths.ledger.as_str(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 100); do echo noise; done; echo ERROR trusted",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["strategy_kind"], "user_toml");
    assert_eq!(json["payload"]["rule_id"], "repo_rule");
    assert_eq!(json["payload"]["strategy_source_kind"], "repo");
}
