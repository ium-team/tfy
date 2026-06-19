use std::process::Command;

fn write_v3_rule(dir: &tempfile::TempDir) -> (String, String) {
    let rule = dir.path().join("commands.toml");
    let fixture = dir.path().join("fixture.txt");
    std::fs::write(
        &rule,
        r#"
schema_version = 3

[[command]]
id = "custom_quality"
match.argv_prefix = ["custom-quality"]
max_lines = 8

[[command.parse_ndjson]]
name = "files"
path = "file"
max_items = 4

[[command.metric]]
name = "errors"
op = "count"
match = "ERROR"

[[command.group]]
name = "by_file"
pattern = 'file=(?<file>[^\s]+)'
field = "file"
top_k = 3
"#,
    )
    .unwrap();
    std::fs::write(
        &fixture,
        (r#"{"file":"src/app.ts","level":"ERROR","secret":"NPM_TOKEN=super-secret-value"}"#
            .to_string()
            + "\nfile=src/app.ts ERROR\nfile=src/lib.ts ERROR\n")
            .repeat(80),
    )
    .unwrap();
    (
        rule.to_str().unwrap().to_string(),
        fixture.to_str().unwrap().to_string(),
    )
}

#[test]
fn rules_validate_accepts_v3_file() {
    let dir = tempfile::tempdir().unwrap();
    let (rule, _) = write_v3_rule(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["rules", "validate", "--file", &rule, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "validate");
}

#[test]
fn rules_preview_uses_raw_first_summary_and_redacts() {
    let dir = tempfile::tempdir().unwrap();
    let (rule, fixture) = write_v3_rule(&dir);
    let raw_dir = dir.path().join("raw").to_str().unwrap().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "preview",
            "--file",
            &rule,
            "--cmd",
            "custom-quality",
            "--fixture",
            &fixture,
            "--exit-code",
            "1",
            "--raw-dir",
            &raw_dir,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let model_text = json["model_text"].as_str().unwrap();
    assert_eq!(json["strategy_kind"], "user_toml");
    assert_eq!(json["rule_id"], "custom_quality");
    assert!(model_text.contains("files:"), "{model_text}");
    assert!(model_text.contains("counter.errors="), "{model_text}");
    assert!(model_text.contains("group.by_file:"), "{model_text}");
    assert!(model_text.contains("raw_ref="), "{model_text}");
    assert!(!model_text.contains("super-secret-value"), "{model_text}");
    assert!(model_text.contains("[REDACTED]"), "{model_text}");
}

#[test]
fn rules_preview_accepts_explicit_argv_for_quoted_commands() {
    let dir = tempfile::tempdir().unwrap();
    let rule = dir.path().join("commands.toml");
    let fixture = dir.path().join("fixture.txt");
    std::fs::write(
        &rule,
        r#"
schema_version = 3

[[command]]
id = "quoted_command"
match.argv_prefix = ["tool", "two words"]
keep_lines_matching = ["KEEP"]
max_lines = 8
"#,
    )
    .unwrap();
    std::fs::write(
        &fixture,
        "noise
KEEP this
"
        .repeat(40),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "preview",
            "--file",
            rule.to_str().unwrap(),
            "--cmd",
            "tool 'two words'",
            "--arg",
            "tool",
            "--arg",
            "two words",
            "--fixture",
            fixture.to_str().unwrap(),
            "--raw-dir",
            dir.path().join("raw").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["strategy_kind"], "user_toml");
    assert_eq!(json["rule_id"], "quoted_command");
}

#[test]
fn rules_agent_workspace_and_trust_are_repo_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["rules", "agent-workspace", "--repo", &repo, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".tfy/commands.toml").exists());
    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "repo_rule"
match.argv_prefix = ["repo-rule"]
keep_lines_matching = ["KEEP"]
"#,
    )
    .unwrap();
    let trust = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["rules", "trust", "--repo", &repo, "--json"])
        .output()
        .unwrap();
    assert!(
        trust.status.success(),
        "{}",
        String::from_utf8_lossy(&trust.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&trust.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert!(dir.path().join(".tfy/trust.json").exists());

    let outside = dir.path().join("outside.toml");
    std::fs::write(&outside, "schema_version = 3\n").unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "trust",
            "--repo",
            &repo,
            "--file",
            outside.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
}

#[test]
fn rules_preview_stores_invalid_utf8_fixture_as_raw_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let rule = dir.path().join("commands.toml");
    let fixture = dir.path().join("fixture.bin");
    std::fs::write(
        &rule,
        r#"
schema_version = 3

[[command]]
id = "binary_preview"
match.argv_prefix = ["binary-preview"]
keep_lines_matching = ["KEEP"]
max_lines = 8
"#,
    )
    .unwrap();
    std::fs::write(&fixture, b"KEEP\xffsecret").unwrap();
    let raw_dir = dir.path().join("raw");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "preview",
            "--file",
            rule.to_str().unwrap(),
            "--cmd",
            "binary-preview",
            "--fixture",
            fixture.to_str().unwrap(),
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["rendering_kind"], "suppressed");
    let raw_ref = json["raw_ref"].as_str().unwrap();
    let raw = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["raw", "--raw-dir", raw_dir.to_str().unwrap(), raw_ref])
        .output()
        .unwrap();
    assert!(raw.status.success());
    assert_eq!(raw.stdout, b"KEEP\xffsecret");
}

#[test]
#[cfg(unix)]
fn rules_trust_rejects_symlinked_tfy_directory() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("actual-tfy")).unwrap();
    std::fs::write(
        outside.path().join("actual-tfy/commands.toml"),
        "schema_version = 3\n",
    )
    .unwrap();
    symlink(outside.path().join("actual-tfy"), dir.path().join(".tfy")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "trust",
            "--repo",
            dir.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!outside.path().join("actual-tfy/trust.json").exists());
}

#[test]
#[cfg(unix)]
fn rules_trust_rejects_symlinked_trust_file() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    std::fs::write(
        tfy.join("commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "safe"
match.argv_prefix = ["safe"]
keep_lines_matching = ["KEEP"]
"#,
    )
    .unwrap();
    let outside_target = outside.path().join("trust-target.json");
    std::fs::write(&outside_target, "do not overwrite").unwrap();
    symlink(&outside_target, tfy.join("trust.json")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "trust",
            "--repo",
            dir.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(&outside_target).unwrap(),
        "do not overwrite"
    );
}
