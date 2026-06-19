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
fn rules_compare_built_in_reports_effective_override_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let rule = dir.path().join("commands.toml");
    let fixture = dir.path().join("df.txt");
    std::fs::write(
        &rule,
        r#"
schema_version = 3

[[command]]
id = "override_df"
match.argv_prefix = ["df"]
keep_lines_matching = ["Filesystem|Use%|/dev/disk1s1"]
max_lines = 4

[command.override]
built_in = true
family = "df"
reason = "agent-authored disk view"

[[command.metric]]
name = "filesystems"
op = "count"
match = "^/dev/"
"#,
    )
    .unwrap();
    std::fs::write(
        &fixture,
        "Filesystem      Size  Used Avail Use% Mounted on
/dev/disk1s1    100G   95G    5G  95% /
"
        .repeat(40),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "rules",
            "compare-built-in",
            "--file",
            rule.to_str().unwrap(),
            "--cmd",
            "df",
            "--arg",
            "df",
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
    let note = json["note"].as_str().unwrap();
    assert!(note.contains("effective custom-rule behavior"), "{note}");
    assert!(!note.contains("override is not enabled"), "{note}");
    assert_eq!(json["comparison"]["override_active"], true);
    assert_eq!(json["comparison"]["with_rules_strategy_kind"], "user_toml");
    assert_eq!(json["comparison"]["without_rules_strategy_kind"], "dsl");
    assert!(json["comparison"]["with_rules_summary_chars"]
        .as_u64()
        .is_some());
    assert!(json["comparison"]["without_rules_summary_chars"]
        .as_u64()
        .is_some());
    assert_eq!(json["with_rules"]["strategy_kind"], "user_toml");
    assert_eq!(json["with_rules"]["rule_id"], "override_df");
    assert_eq!(json["without_rules"]["strategy_kind"], "dsl");
    let diagnostics = json["with_rules"]["command_rule_diagnostics"]
        .as_array()
        .unwrap();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic["code"] == "user_rule_overrode_builtin"));
}

#[test]
fn rules_preview_reports_explicit_built_in_override() {
    let dir = tempfile::tempdir().unwrap();
    let rule = dir.path().join("commands.toml");
    let fixture = dir.path().join("df.txt");
    std::fs::write(
        &rule,
        r#"
schema_version = 3

[[command]]
id = "override_df"
match.argv_prefix = ["df"]
keep_lines_matching = ["Filesystem|Use%|/dev/disk1s1"]
max_lines = 4

[command.override]
built_in = true
family = "df"
reason = "agent-authored disk view"

[[command.metric]]
name = "filesystems"
op = "count"
match = "^/dev/"
"#,
    )
    .unwrap();
    std::fs::write(
        &fixture,
        "Filesystem      Size  Used Avail Use% Mounted on
/dev/disk1s1    100G   95G    5G  95% /
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
            "df",
            "--arg",
            "df",
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
    assert_eq!(json["command_family"], "df");
    assert_eq!(json["strategy_kind"], "user_toml");
    assert_eq!(json["rule_id"], "override_df");
    assert!(json["model_text"]
        .as_str()
        .unwrap()
        .contains("counter.filesystems="));
    let diagnostics = json["command_rule_diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic["code"] == "user_rule_overrode_builtin"));
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
    assert!(!trust.status.success());
    let stderr = String::from_utf8_lossy(&trust.stderr);
    assert!(stderr.contains("tfy rules trust is deprecated"), "{stderr}");
    assert!(
        stderr.contains("tfy custom verify --scope repo"),
        "{stderr}"
    );
    assert!(!dir.path().join(".tfy/trust.json").exists());

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

#[test]
fn custom_init_capture_verify_trust_and_status_write_v2_trust() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();

    let init = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo, "--json"])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(dir
        .path()
        .join(".tfy/custom/AGENT_INSTRUCTIONS.md")
        .exists());

    let capture = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "capture",
            "--repo",
            repo,
            "--name",
            "quality",
            "--json",
            "--",
            "sh",
            "-c",
            "for i in 1 2 3 4 5 6 7 8 9 10; do echo noise; echo KEEP; done",
        ])
        .output()
        .unwrap();
    assert!(
        capture.status.success(),
        "{}",
        String::from_utf8_lossy(&capture.stderr)
    );
    assert!(dir.path().join(".tfy/rule-fixtures/quality.txt").exists());
    assert!(dir.path().join(".tfy/custom/quality.json").exists());

    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "quality_rule"
match.argv_prefix = ["sh", "-c"]
keep_lines_matching = ["KEEP"]
max_lines = 4
"#,
    )
    .unwrap();

    let verify = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom", "verify", "--repo", repo, "--name", "quality", "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let verify_json: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(verify_json["ok"], true);
    assert_eq!(verify_json["validated_with"][0], "validate");
    assert_eq!(verify_json["preview_strategy_kind"], "user_toml");

    let trust = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom", "trust", "--repo", repo, "--name", "quality", "--json",
        ])
        .output()
        .unwrap();
    assert!(
        trust.status.success(),
        "{}",
        String::from_utf8_lossy(&trust.stderr)
    );
    let trust_json: serde_json::Value = serde_json::from_slice(&trust.stdout).unwrap();
    assert_eq!(trust_json["schema_version"], 2);
    assert_eq!(trust_json["command_rules"]["created_by"], "tfy custom");
    assert_eq!(
        trust_json["command_rules"]["agent"]["bounded_workspace"],
        true
    );

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "status", "--repo", repo, "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let status_json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status_json["repo_local"]["state"], "trusted_v2");
    assert_eq!(status_json["repo_local"]["trust_schema_version"], 2);
    assert_eq!(status_json["user_global_legacy"]["state"], "missing");
    assert_eq!(status_json["global_custom"]["state"], "missing");

    std::fs::write(
        dir.path().join(".tfy/rule-fixtures/quality.txt"),
        "tampered
",
    )
    .unwrap();
    let stale_status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "status", "--repo", repo, "--json"])
        .output()
        .unwrap();
    assert!(stale_status.status.success());
    let stale_json: serde_json::Value = serde_json::from_slice(&stale_status.stdout).unwrap();
    assert_eq!(stale_json["repo_local"]["state"], "stale");

    let mut v1_trust = trust_json;
    v1_trust["schema_version"] = serde_json::json!(1);
    v1_trust["command_rules"]["rules_sha256"] = serde_json::json!("stale-v1-hash");
    std::fs::write(
        dir.path().join(".tfy/trust.json"),
        serde_json::to_string_pretty(&v1_trust).unwrap(),
    )
    .unwrap();
    let v1_status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "status", "--repo", repo, "--json"])
        .output()
        .unwrap();
    assert!(v1_status.status.success());
    let v1_json: serde_json::Value = serde_json::from_slice(&v1_status.stdout).unwrap();
    assert_eq!(v1_json["repo_local"]["state"], "untrusted");
}

#[test]
fn custom_global_wizard_initializes_global_store_without_repo_tfy() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .arg("custom")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(b"global\n")?;
            child.wait_with_output()
        })
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("global scope selected"));
    assert!(home
        .path()
        .join(".config/tfy/custom/commands.toml")
        .exists());
    assert!(home
        .path()
        .join(".config/tfy/custom/rule-fixtures")
        .exists());
    assert!(home
        .path()
        .join(".config/tfy/custom/agent/AGENT_INSTRUCTIONS.md")
        .exists());
    assert!(!home.path().join(".config/tfy/commands.toml").exists());
    assert!(!dir.path().join(".tfy").exists());
}

#[test]
fn tool_gateway_loads_trusted_global_custom_rule_across_repos_and_repo_untrusted_does_not_block() {
    let global_author_repo = tempfile::tempdir().unwrap();
    let run_repo = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_tfy");
    let global_author_arg = global_author_repo.path().to_str().unwrap();

    Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "init",
            "--scope",
            "global",
            "--repo",
            global_author_arg,
        ])
        .status()
        .unwrap();
    let fixture = global_author_repo.path().join("fixture.txt");
    std::fs::write(&fixture, "noise\nKEEP global\n".repeat(120)).unwrap();
    Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "import-fixture",
            "--scope",
            "global",
            "--repo",
            global_author_arg,
            "--name",
            "runtimeglobal",
            "--file",
            fixture.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    let global_root = home.path().join(".config/tfy/custom");
    std::fs::write(
        global_root.join("commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "runtime_global_rule"
match.argv_prefix = ["sh", "-c"]
keep_lines_matching = ["KEEP global"]
max_lines = 4
"#,
    )
    .unwrap();
    let meta_path = global_root.join("agent/runtimeglobal.json");
    let mut meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    meta["cmd"] = serde_json::json!([
        "sh",
        "-c",
        "for i in 1 2 3 4 5 6 7 8 9 10; do echo noise; echo KEEP global; done"
    ]);
    meta["command_display"] = serde_json::json!("sh -c for i in ...");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let verify = Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "verify",
            "--scope",
            "global",
            "--repo",
            global_author_arg,
            "--name",
            "runtimeglobal",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let trust = Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "trust",
            "--scope",
            "global",
            "--repo",
            global_author_arg,
            "--name",
            "runtimeglobal",
        ])
        .output()
        .unwrap();
    assert!(
        trust.status.success(),
        "{}",
        String::from_utf8_lossy(&trust.stderr)
    );

    // A bad repo-local source must not short-circuit the later trusted global source.
    std::fs::create_dir_all(run_repo.path().join(".tfy")).unwrap();
    std::fs::write(
        run_repo.path().join(".tfy/commands.toml"),
        "schema_version = 3\n",
    )
    .unwrap();

    let output = Command::new(bin)
        .current_dir(run_repo.path())
        .env("HOME", home.path())
        .args([
            "tool-gateway",
            "--json",
            "--",
            "sh",
            "-c",
            "for i in 1 2 3 4 5 6 7 8 9 10; do echo noise; echo KEEP global; done",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("repo_rules_untrusted"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("runtime_global_rule"), "{stdout}");
    assert!(stdout.contains("global_custom"), "{stdout}");

    let status = Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "status",
            "--repo",
            run_repo.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(status.status.success());
    let status_json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status_json["global_custom"]["state"], "trusted_v2");
    assert_eq!(status_json["user_global_legacy"]["state"], "missing");

    std::fs::write(
        global_root.join("rule-fixtures/runtimeglobal.txt"),
        "tampered
",
    )
    .unwrap();
    let stale_status = Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "status",
            "--repo",
            run_repo.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(stale_status.status.success());
    let stale_json: serde_json::Value = serde_json::from_slice(&stale_status.stdout).unwrap();
    assert_eq!(stale_json["global_custom"]["state"], "stale");
}

#[test]
fn custom_verify_fails_by_default_when_user_global_rules_exist() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".config/tfy");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("commands.toml"), "schema_version = 3\n").unwrap();
    let repo = dir.path().to_str().unwrap();

    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo])
        .status()
        .unwrap();
    std::fs::write(dir.path().join("sample.txt"), "KEEP\n".repeat(80)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "import-fixture",
            "--repo",
            repo,
            "--name",
            "sample",
            "--file",
            dir.path().join("sample.txt").to_str().unwrap(),
        ])
        .status()
        .unwrap();
    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "sample_rule"
match.argv_prefix = ["sample"]
keep_lines_matching = ["KEEP"]
"#,
    )
    .unwrap();

    let rejected = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "verify", "--repo", repo, "--name", "sample"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--allow-legacy-global-rules"));

    let accepted = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "verify",
            "--repo",
            repo,
            "--name",
            "sample",
            "--allow-legacy-global-rules",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(json["user_global_rules_legacy_manual"], true);
    assert_eq!(json["allow_legacy_global_rules"], true);
}

#[test]
fn tool_gateway_loads_v2_trusted_repo_custom_rule_and_diagnoses_global_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_tfy");

    Command::new(bin)
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo])
        .status()
        .unwrap();
    let fixture = dir.path().join("fixture.txt");
    std::fs::write(&fixture, "noise\nKEEP runtime\n".repeat(120)).unwrap();
    Command::new(bin)
        .env("HOME", home.path())
        .args([
            "custom",
            "import-fixture",
            "--repo",
            repo,
            "--name",
            "runtime",
            "--file",
            fixture.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "runtime_rule"
match.argv_prefix = ["sh", "-c"]
keep_lines_matching = ["KEEP runtime"]
max_lines = 4
"#,
    )
    .unwrap();
    let meta_path = dir.path().join(".tfy/custom/runtime.json");
    let mut meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    meta["cmd"] = serde_json::json!([
        "sh",
        "-c",
        "for i in 1 2 3 4 5 6 7 8 9 10; do echo noise; echo KEEP runtime; done"
    ]);
    meta["command_display"] = serde_json::json!("sh -c for i in ...");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let verify = Command::new(bin)
        .env("HOME", home.path())
        .args(["custom", "verify", "--repo", repo, "--name", "runtime"])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let trust = Command::new(bin)
        .env("HOME", home.path())
        .args(["custom", "trust", "--repo", repo, "--name", "runtime"])
        .output()
        .unwrap();
    assert!(
        trust.status.success(),
        "{}",
        String::from_utf8_lossy(&trust.stderr)
    );

    let config = home.path().join(".config/tfy");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("commands.toml"), "schema_version = 3\n").unwrap();
    let output = Command::new(bin)
        .current_dir(dir.path())
        .env("HOME", home.path())
        .args([
            "tool-gateway",
            "--json",
            "--",
            "sh",
            "-c",
            "for i in 1 2 3 4 5 6 7 8 9 10; do echo noise; echo KEEP runtime; done",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("user_global_rules_legacy_manual"),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user_toml"), "{stdout}");
    assert!(stdout.contains("runtime_rule"), "{stdout}");
}

#[test]
fn custom_verify_rejects_fixture_that_does_not_match_user_rule() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    let sample = dir.path().join("sample.txt");
    std::fs::write(&sample, "noise\n".repeat(120)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo])
        .status()
        .unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "import-fixture",
            "--repo",
            repo,
            "--name",
            "sample",
            "--file",
            sample.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "other_rule"
match.argv_prefix = ["other"]
keep_lines_matching = ["KEEP"]
"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "verify", "--repo", repo, "--name", "sample"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not exercise a user TOML rule"));
}

#[test]
fn custom_capture_rejects_secret_like_args_and_create_json_is_single_document() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "capture",
            "--repo",
            repo,
            "--name",
            "secret",
            "--",
            "echo",
            "Authorization: Bearer abc123",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("secret-like command arguments"));

    let opaque = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "capture",
            "--repo",
            repo,
            "--name",
            "opaque",
            "--",
            "echo",
            "aB3dE5gH7jK9mN2pQ4rS6tU8",
        ])
        .output()
        .unwrap();
    assert!(!opaque.status.success());
    assert!(String::from_utf8_lossy(&opaque.stderr).contains("opaque high-entropy"));

    let created = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "create",
            "--repo",
            repo,
            "--name",
            "onejson",
            "--agent",
            "codex",
            "--json",
            "--",
            "sh",
            "-c",
            "printf KEEP",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let stdout = String::from_utf8_lossy(&created.stdout);
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.trim_end().ends_with('}'), "{stdout}");
    let json: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "create");

    let trusted_repo = tempfile::tempdir().unwrap();
    let trusted_repo_arg = trusted_repo.path().to_str().unwrap();
    std::fs::create_dir_all(trusted_repo.path().join(".tfy")).unwrap();
    std::fs::write(
        trusted_repo.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "create_rule"
match.argv_prefix = ["sh", "-c"]
keep_lines_matching = ["KEEP"]
max_lines = 4
"#,
    )
    .unwrap();
    let trusted = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "create",
            "--repo",
            trusted_repo_arg,
            "--name",
            "trustedjson",
            "--agent",
            "codex",
            "--verify-and-trust",
            "--json",
            "--",
            "sh",
            "-c",
            "for i in 1 2 3 4 5 6 7 8; do echo noise; echo KEEP; done",
        ])
        .output()
        .unwrap();
    assert!(
        trusted.status.success(),
        "{}",
        String::from_utf8_lossy(&trusted.stderr)
    );
    let trusted_json: serde_json::Value = serde_json::from_slice(&trusted.stdout).unwrap();
    assert_eq!(trusted_json["ok"], true);
    assert_eq!(trusted_json["stopped_before_trust"], false);
    assert_eq!(trusted_json["verify"]["preview_strategy_kind"], "user_toml");
    assert_eq!(trusted_json["trust"]["schema_version"], 2);
}

#[test]
#[cfg(unix)]
fn custom_verify_rejects_symlinked_raw_dir() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    let sample = dir.path().join("sample.txt");
    std::fs::write(&sample, "KEEP\n".repeat(120)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo])
        .status()
        .unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args([
            "custom",
            "import-fixture",
            "--repo",
            repo,
            "--name",
            "sample",
            "--file",
            sample.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    std::fs::write(
        dir.path().join(".tfy/commands.toml"),
        r#"
schema_version = 3

[[command]]
id = "sample_rule"
match.argv_prefix = ["sample"]
keep_lines_matching = ["KEEP"]
"#,
    )
    .unwrap();
    let raw = dir.path().join(".tfy/custom/raw");
    std::fs::create_dir_all(raw.parent().unwrap()).unwrap();
    symlink(outside.path(), &raw).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "verify", "--repo", repo, "--name", "sample"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("refuses symlink path"));
}

#[test]
#[cfg(unix)]
fn custom_init_rejects_broken_symlinked_agent_template() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let custom_dir = repo.join(".tfy/custom");
    std::fs::create_dir_all(&custom_dir).unwrap();
    let outside_target = outside.path().join("outside-agent-instructions.md");
    symlink(&outside_target, custom_dir.join("AGENT_INSTRUCTIONS.md")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("HOME", home.path())
        .args(["custom", "init", "--repo", repo.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!outside_target.exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("refuses symlink path"));
}

#[test]
fn bare_custom_wizard_repo_choice_initializes_repo_harness() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .arg("custom")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(b"repo\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".tfy/commands.toml").exists());
    assert!(dir.path().join(".tfy/rule-fixtures").is_dir());
    assert!(dir
        .path()
        .join(".tfy/custom/AGENT_INSTRUCTIONS.md")
        .exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("repo scope selected"));
}

#[test]
fn bare_custom_wizard_global_choice_initializes_global_store() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .arg("custom")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(b"global\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("global scope selected"));
    assert!(home
        .path()
        .join(".config/tfy/custom/commands.toml")
        .exists());
    assert!(!dir.path().join(".tfy/commands.toml").exists());
}

#[test]
fn bare_custom_wizard_empty_non_tty_input_fails_closed_without_repo_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .arg("custom")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!dir.path().join(".tfy/commands.toml").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("explicit scope choice"));
}
