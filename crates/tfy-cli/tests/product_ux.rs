use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn append_smoke_launch_args(args: &mut Vec<String>, smoke_json: &serde_json::Value) {
    for entry in smoke_json["evidence"].as_array().unwrap() {
        let text = entry.as_str().unwrap();
        if let Some(path) = text.strip_prefix("ledger=") {
            args.push("--ledger".into());
            args.push(path.to_string());
        } else if let Some(path) = text.strip_prefix("host_evidence=") {
            args.push("--host-evidence".into());
            args.push(path.to_string());
        }
    }
}

fn write_release_evidence_fixture(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let root = dir.path();
    let files = [
        "install-bin",
        "release-bin",
        "archive.tar.gz",
        "archive.sha256",
        "docs.md",
        "notes.md",
        "review.json",
        "ci.json",
    ];
    for file in files {
        std::fs::write(root.join(file), "fixture evidence").unwrap();
    }
    std::fs::write(
        root.join("bench-manifest.json"),
        r#"{
          "status": "pass",
          "tfy_self_benchmark": {
            "no_negative_savings": true,
            "positive_savings": true
          },
          "scenarios": [
            {"raw_ref": "cmdout_fixture", "no_negative_savings": true}
          ]
        }"#,
    )
    .unwrap();
    let release_evidence = root.join("release-evidence.json");
    std::fs::write(
        &release_evidence,
        r#"{
          "cargo_install_verified": true,
          "cargo_install_binary": "install-bin",
          "cargo_build_release_verified": true,
          "release_binary": "release-bin",
          "archive_checksum_dry_run": true,
          "archive_artifact": "archive.tar.gz",
          "checksum_artifact": "archive.sha256",
          "docs_demo_release_notes_complete": true,
          "docs_artifact": "docs.md",
          "release_notes_artifact": "notes.md",
          "independent_reviews_approved": true,
          "review_artifact": "review.json",
          "pr_ci_green": true,
          "ci_artifact": "ci.json",
          "benchmark_manifest_generated": true,
          "benchmark_manifest": "bench-manifest.json",
          "notes": ["test fixture artifact-backed release evidence"]
        }"#,
    )
    .unwrap();
    release_evidence
}

#[test]
fn init_codex_dry_run_writes_nothing_and_prints_tiers() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!agents.exists());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("dry_run=true"), "{text}");
    assert!(text.contains("mcp_host_routed"), "{text}");
    assert!(text.contains("instruction_guidance"), "{text}");
    assert!(text.contains("TFY:CODEX:START"), "{text}");
    assert!(
        text.contains("codex mcp add tfy -- tfy mcp serve"),
        "{text}"
    );
    assert!(text.contains("no private_hook"), "{text}");
}

#[test]
fn init_project_apply_is_idempotent_and_uninstall_preserves_non_tfy_content() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    std::fs::write(&agents, "# Existing\nkeep me\n").unwrap();

    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .current_dir(dir.path())
            .args(["init", "--codex", "--project", "--apply"])
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    let text = std::fs::read_to_string(&agents).unwrap();
    assert_eq!(text.matches("TFY:CODEX:START").count(), 1, "{text}");
    assert!(text.contains("keep me"), "{text}");
    assert!(text.contains("private_hook: not claimed"), "{text}");

    let show = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--show"])
        .output()
        .unwrap();
    assert!(show.status.success());
    let show_json: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(show_json["targets"][0]["installed"], true);

    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--uninstall", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(uninstall.status.success());
    let text = std::fs::read_to_string(&agents).unwrap();
    assert!(text.contains("keep me"), "{text}");
    assert!(!text.contains("TFY:CODEX:START"), "{text}");
}

#[test]
fn init_project_apply_replaces_duplicate_marker_blocks_without_touching_surrounding_text() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    std::fs::write(
        &agents,
        "# Existing\nalpha\n<!-- TFY:CODEX:START -->\nold one\n<!-- TFY:CODEX:END -->\nbeta\n<!-- TFY:CODEX:START -->\nold two\n<!-- TFY:CODEX:END -->\ngamma\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(&agents).unwrap();
    assert_eq!(text.matches("TFY:CODEX:START").count(), 1, "{text}");
    assert!(text.contains("# Existing\nalpha\n"), "{text}");
    assert!(text.contains("\nbeta\n"), "{text}");
    assert!(text.contains("\ngamma\n"), "{text}");
    assert!(!text.contains("old one"), "{text}");
    assert!(!text.contains("old two"), "{text}");

    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--uninstall", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(uninstall.status.success());
    let text = std::fs::read_to_string(&agents).unwrap();
    assert!(text.contains("# Existing\nalpha\n"), "{text}");
    assert!(text.contains("\nbeta\n"), "{text}");
    assert!(text.contains("\ngamma\n"), "{text}");
    assert!(!text.contains("TFY:CODEX:"), "{text}");
}

#[test]
fn init_project_apply_fails_closed_on_malformed_marker_block() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    let original = "# Existing\n<!-- TFY:CODEX:START -->\nmissing end\n";
    std::fs::write(&agents, original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&agents).unwrap(), original);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("malformed TFY Codex marker block"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn init_project_apply_fails_closed_on_orphan_end_marker_before_valid_block() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    let original = "# Existing\n<!-- TFY:CODEX:END -->\nalpha\n<!-- TFY:CODEX:START -->\nold\n<!-- TFY:CODEX:END -->\n";
    std::fs::write(&agents, original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&agents).unwrap(), original);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("end marker without start marker"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn init_project_apply_fails_closed_on_orphan_end_marker_between_valid_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    let original = "# Existing\n<!-- TFY:CODEX:START -->\none\n<!-- TFY:CODEX:END -->\nalpha\n<!-- TFY:CODEX:END -->\nbeta\n<!-- TFY:CODEX:START -->\ntwo\n<!-- TFY:CODEX:END -->\n";
    std::fs::write(&agents, original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&agents).unwrap(), original);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("end marker without start marker"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn init_project_apply_fails_closed_on_unreadable_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("AGENTS.md");
    let original = vec![0xff, 0xfe, b'A'];
    std::fs::write(&agents, &original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["init", "--codex", "--project", "--apply"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&agents).unwrap(), original);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("read AGENTS.md"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_json_checks_local_mcp_and_codex_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["doctor", "--codex", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(json["status"], "fail");
    let diagnostics = json["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["name"] == "mcp_initialize" && d["status"] == "pass"));
    assert!(diagnostics
        .iter()
        .any(|d| d["name"] == "mcp_tools" && d["status"] == "pass"));
    assert!(diagnostics
        .iter()
        .any(|d| d["name"] == "codex_mcp_config" && d["status"] == "warn"));
}

#[test]
fn smoke_mcp_exercises_code_io_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--mcp", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "pass");
    assert_eq!(json["preview_applied"], false);
    assert_eq!(json["apply_applied"], true);
    assert!(json["ledger_events"].as_u64().unwrap() >= 4, "{json}");
    assert!(
        json["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("tfy_state_project")),
        "{json}"
    );
}

#[test]
fn smoke_all_produces_adapter_and_mcp_launch_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "pass");
    let modes: Vec<_> = json["reports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["mode"].as_str().unwrap())
        .collect();
    assert!(modes.contains(&"adapter"), "{json}");
    assert!(modes.contains(&"agent"), "{json}");
    assert!(modes.contains(&"mcp"), "{json}");
    assert!(
        json["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry.as_str().unwrap().starts_with("host_evidence=")),
        "{json}"
    );
}

#[test]
fn smoke_codex_is_checklist_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["smoke", "--codex"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("checklist/report-only"), "{text}");
    assert!(
        text.contains("does not claim Codex host invocation"),
        "{text}"
    );
}

#[test]
fn setup_named_hosts_emit_truthful_mcp_snippets_without_claiming_savings() {
    let cases = [
        ("codex", "codex mcp add tfy"),
        ("claude-code", "claude mcp add tfy"),
        ("cursor", "\"mcpServers\""),
        ("opencode", "\"mcp\""),
        ("hermes", "mcp_servers:"),
    ];
    for (host, expected) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .args(["setup", "--ai", "--host", host, "--dry-run"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "host={host} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(expected), "host={host} stdout={text}");
        assert!(
            text.contains("setup_success_is_not_savings_success=true"),
            "host={host} stdout={text}"
        );
        assert!(
            text.contains("no private hidden hooks"),
            "host={host} stdout={text}"
        );
    }
}

#[test]
fn setup_openclaw_remains_planned_discovery() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["setup", "--ai", "--host", "openclaw", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("planned_discovery"), "{text}");
    assert!(text.contains("no setup/apply/smoke support"), "{text}");
}

#[test]
fn setup_cursor_project_apply_is_reversible_and_preserves_existing_config() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join(".cursor");
    std::fs::create_dir_all(&cursor_dir).unwrap();
    let config = cursor_dir.join("mcp.json");
    std::fs::write(
        &config,
        r#"{"mcpServers":{"other":{"command":"other"}},"keep":true}"#,
    )
    .unwrap();

    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .current_dir(dir.path())
            .args(["setup", "--ai", "--host", "cursor", "--apply", "--project"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(cursor_dir.join("mcp.json.tfy-backup").exists());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    assert_eq!(json["keep"], true);
    assert_eq!(json["mcpServers"]["other"]["command"], "other");
    assert_eq!(json["mcpServers"]["tfy"]["command"], "tfy");
    assert_eq!(json["mcpServers"]["tfy"]["tfy_managed"], true);
    assert_eq!(json["mcpServers"]["tfy"].as_object().unwrap().len(), 3);
    assert_eq!(
        std::fs::read_to_string(cursor_dir.join("mcp.json.tfy-backup")).unwrap(),
        r#"{"mcpServers":{"other":{"command":"other"}},"keep":true}"#
    );

    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "setup",
            "--ai",
            "--host",
            "cursor",
            "--uninstall",
            "--apply",
            "--project",
        ])
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    assert_eq!(json["mcpServers"]["other"]["command"], "other");
    assert!(json["mcpServers"]["tfy"].is_null(), "{json}");
    assert_eq!(
        std::fs::read_to_string(cursor_dir.join("mcp.json.tfy-backup")).unwrap(),
        r#"{"mcpServers":{"other":{"command":"other"}},"keep":true}"#
    );
}

#[test]
fn setup_cursor_project_apply_refuses_to_overwrite_unowned_tfy_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join(".cursor");
    std::fs::create_dir_all(&cursor_dir).unwrap();
    let config = cursor_dir.join("mcp.json");
    let original = r#"{"mcpServers":{"tfy":{"command":"custom-tfy","args":["custom"]}}}"#;
    std::fs::write(&config, original).unwrap();

    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["setup", "--ai", "--host", "cursor", "--apply", "--project"])
        .output()
        .unwrap();
    assert!(!apply.status.success());
    assert_eq!(std::fs::read_to_string(&config).unwrap(), original);
    assert!(!cursor_dir.join("mcp.json.tfy-backup").exists());

    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "setup",
            "--ai",
            "--host",
            "cursor",
            "--uninstall",
            "--apply",
            "--project",
        ])
        .output()
        .unwrap();
    assert!(!uninstall.status.success());
    assert_eq!(std::fs::read_to_string(&config).unwrap(), original);
}

#[test]
fn setup_cursor_project_apply_fails_closed_on_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join(".cursor");
    std::fs::create_dir_all(&cursor_dir).unwrap();
    let config = cursor_dir.join("mcp.json");
    let original = "{not-json";
    std::fs::write(&config, original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["setup", "--ai", "--host", "cursor", "--apply", "--project"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&config).unwrap(), original);
    assert!(!cursor_dir.join("mcp.json.tfy-backup").exists());
}

#[test]
fn setup_cursor_project_dry_run_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "setup",
            "--ai",
            "--host",
            "cursor",
            "--apply",
            "--project",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("status=dry_run claim_tier=configurable"),
        "{text}"
    );
    assert!(text.contains("dry_run=true applied=false"), "{text}");
    assert!(!dir.path().join(".cursor").join("mcp.json").exists());
}

#[test]
fn setup_cursor_project_uninstall_without_existing_config_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "setup",
            "--ai",
            "--host",
            "cursor",
            "--uninstall",
            "--apply",
            "--project",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join(".cursor").join("mcp.json").exists());
}

#[test]
fn setup_cursor_project_uninstall_removes_tfy_created_config_without_backup() {
    let dir = tempfile::tempdir().unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["setup", "--ai", "--host", "cursor", "--apply", "--project"])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "setup",
            "--ai",
            "--host",
            "cursor",
            "--uninstall",
            "--apply",
            "--project",
        ])
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(!dir.path().join(".cursor").join("mcp.json").exists());
    assert!(!dir
        .path()
        .join(".cursor")
        .join("mcp.json.tfy-backup")
        .exists());
}

#[test]
fn mcp_install_supports_named_host_dry_runs_and_blocks_non_codex_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["mcp", "install", "--target", "hermes", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("TFY MCP Hermes setup dry-run"), "{text}");
    assert!(text.contains("mcp_servers:"), "{text}");
    assert!(
        text.contains("Setup success is not token-savings success"),
        "{text}"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cursor.json");
    let blocked = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "mcp",
            "install",
            "--target",
            "cursor",
            "--output",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(!path.exists());
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("Codex TOML only"),
        "stderr={}",
        String::from_utf8_lossy(&blocked.stderr)
    );
}

#[test]
fn status_json_exposes_canonical_named_host_taxonomy() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let hosts = json["minimum_v1_host_matrix"].as_array().unwrap();
    let find = |name: &str| {
        hosts
            .iter()
            .find(|host| host["host"] == name)
            .unwrap_or_else(|| panic!("missing host {name}: {json}"))
    };
    assert_eq!(find("codex")["status"], "config_snippet_available");
    assert_eq!(find("claude-code")["status"], "config_snippet_available");
    assert_eq!(find("cursor")["status"], "config_snippet_available");
    assert_eq!(find("opencode")["status"], "config_snippet_available");
    assert_eq!(find("hermes")["status"], "config_snippet_available");
    assert_eq!(find("openclaw")["status"], "planned_discovery");
    assert_eq!(find("codex")["claim_tier"], "configurable");
    assert_eq!(find("cursor")["claim_tier"], "configurable");
    assert_eq!(find("openclaw")["claim_tier"], "planned_discovery");
    assert!(json["claim_evidence_ladder"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tier| tier == "verified_host_hook"));
    assert_eq!(find("codex")["next_evidence_tier"], "config_written");
    assert!(find("codex")["supported_ingress"]
        .as_array()
        .unwrap()
        .iter()
        .any(|ingress| ingress == "mcp_stdio"));
    assert!(find("codex")["unsupported_ingress"]
        .as_array()
        .unwrap()
        .iter()
        .any(|ingress| ingress == "private_codex_hook_interception"));
}

#[test]
fn launch_report_keeps_named_host_setup_evidence_below_launch_supported() {
    let dir = tempfile::tempdir().unwrap();
    let setup = dir.path().join("setup.txt");
    let invocation = dir.path().join("invoke.txt");
    std::fs::write(&setup, "configured").unwrap();
    std::fs::write(&invocation, "called").unwrap();
    let evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &evidence,
        r#"{"hosts":[{"host":"cursor","tfy_version":"0.1.0","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup.txt","invocation_artifact":"invoke.txt","overhead_ms":10,"baseline_ms":10}]}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert!(
        json["blockers"].as_array().unwrap().iter().any(|b| b
            .as_str()
            .unwrap()
            .contains("no command-output savings data")),
        "{json}"
    );
}

#[test]
fn launch_report_keeps_openclaw_planned_discovery_even_with_host_evidence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("setup-proof.txt"), "configured").unwrap();
    std::fs::write(dir.path().join("invocation-proof.txt"), "invoked").unwrap();
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {"host":"openclaw","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":10,"baseline_ms":10}
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let openclaw = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "openclaw")
        .unwrap();
    assert_eq!(openclaw["status"], "planned_discovery", "{json}");
    assert_eq!(
        openclaw["launch_claim"],
        "unsupported/planned until official/current evidence proves a safe route",
        "{json}"
    );
}

#[test]
fn launch_report_promotes_named_host_only_with_host_bound_evidence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    for file in [
        "setup.txt",
        "invoke.txt",
        ".cursor/mcp.json",
        "cursor-ledger.jsonl",
        "raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "evidence").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10,
              "host_id":"cursor",
              "host_version":"test",
              "config_scope":"project",
              "config_path":".cursor/mcp.json",
              "route_type":"mcp",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"raw.txt",
              "redacted_public_bytes":200,
              "model_visible_bytes":100,
              "savings_result":"positive",
              "timestamp":"2026-06-09T00:00:00Z",
              "smoke_id":"cursor-smoke-1"
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_eq!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(cursor["claim_tier"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["host_bound_evidence"], true,
        "{json}"
    );
}

#[test]
fn launch_report_keeps_setup_only_named_evidence_from_poisoning_byte_proof() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    for file in [
        "setup.txt",
        "invoke.txt",
        ".cursor/mcp.json",
        "cursor-ledger.jsonl",
        "raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "evidence").unwrap();
    }
    let setup_only = dir.path().join("setup-only.json");
    std::fs::write(
        &setup_only,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10
            }
          ]
        }"#,
    )
    .unwrap();
    let byte_proof = dir.path().join("byte-proof.json");
    std::fs::write(
        &byte_proof,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10,
              "host_id":"cursor",
              "host_version":"test",
              "config_scope":"project",
              "config_path":".cursor/mcp.json",
              "route_type":"mcp",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"raw.txt",
              "redacted_public_bytes":200,
              "model_visible_bytes":100,
              "timestamp":"2026-06-09T00:00:00Z",
              "smoke_id":"cursor-smoke-1"
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            setup_only.to_str().unwrap(),
            "--host-evidence",
            byte_proof.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_eq!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["no_negative_savings"], true,
        "{json}"
    );
}

#[test]
fn launch_report_does_not_promote_named_host_when_bytes_contradict_positive_label() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    for file in [
        "setup.txt",
        "invoke.txt",
        ".cursor/mcp.json",
        "cursor-ledger.jsonl",
        "raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "evidence").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10,
              "host_id":"cursor",
              "host_version":"test",
              "config_scope":"project",
              "config_path":".cursor/mcp.json",
              "route_type":"mcp",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"raw.txt",
              "redacted_public_bytes":100,
              "model_visible_bytes":200,
              "savings_result":"positive",
              "timestamp":"2026-06-09T00:00:00Z",
              "smoke_id":"cursor-smoke-1"
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["host_bound_evidence"], false,
        "{json}"
    );
}

#[test]
fn launch_report_rejects_positive_label_without_byte_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    for file in [
        "setup.txt",
        "invoke.txt",
        ".cursor/mcp.json",
        "cursor-ledger.jsonl",
        "raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "evidence").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10,
              "host_id":"cursor",
              "host_version":"test",
              "config_scope":"project",
              "config_path":".cursor/mcp.json",
              "route_type":"mcp",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"raw.txt",
              "savings_result":"positive",
              "timestamp":"2026-06-09T00:00:00Z",
              "smoke_id":"cursor-smoke-1"
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["host_bound_evidence"], false,
        "{json}"
    );
}

#[test]
fn launch_report_rejects_substring_positive_savings_labels() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    for file in [
        "setup.txt",
        "invoke.txt",
        ".cursor/mcp.json",
        "cursor-ledger.jsonl",
        "raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "evidence").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup.txt",
              "invocation_artifact":"invoke.txt",
              "overhead_ms":10,
              "baseline_ms":10,
              "host_id":"cursor",
              "host_version":"test",
              "config_scope":"project",
              "config_path":".cursor/mcp.json",
              "route_type":"mcp",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"raw.txt",
              "savings_result":"not_positive",
              "timestamp":"2026-06-09T00:00:00Z",
              "smoke_id":"cursor-smoke-1"
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["host_bound_evidence"], false,
        "{json}"
    );
}

#[test]
fn launch_report_caps_named_host_even_with_unrelated_mcp_savings() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for entry in smoke_json["evidence"].as_array().unwrap() {
        let text = entry.as_str().unwrap();
        if let Some(path) = text.strip_prefix("ledger=") {
            owned_args.push("--ledger".into());
            owned_args.push(path.to_string());
        }
    }
    std::fs::write(dir.path().join("setup-proof.txt"), "configured").unwrap();
    std::fs::write(dir.path().join("invocation-proof.txt"), "invoked").unwrap();
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {"host":"cursor","tfy_version":"0.1.0","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":10,"baseline_ms":10}
          ]
        }"#,
    )
    .unwrap();
    owned_args.push("--host-evidence".into());
    owned_args.push(host_evidence.display().to_string());
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["host_evidence"]["positive_savings"], true, "{json}");
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "cursor")
        .unwrap();
    assert_eq!(cursor["status"], "verified_host_invocation", "{json}");
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert!(
        cursor["evidence_gate"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gate| gate
                .as_str()
                .unwrap()
                .contains("observed route_commands=0 route_raw_refs=0")),
        "{json}"
    );
}

#[test]
fn smoke_codex_with_mcp_json_keeps_stdout_machine_readable() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--codex", "--mcp", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "pass");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checklist/report-only"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn gain_empty_and_adapter_ledger_report_real_data() {
    let dir = tempfile::tempdir().unwrap();
    let empty = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["gain"])
        .output()
        .unwrap();
    assert!(empty.status.success());
    let text = String::from_utf8_lossy(&empty.stdout);
    assert!(text.contains("No TFY savings data found yet"), "{text}");
    assert!(text.contains("tfy adapter run -- <command>"), "{text}");
    assert!(text.contains("tfy_tool_run"), "{text}");
    assert!(!text.contains("smoke --mcp"), "{text}");

    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let run = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "adapter",
            "run",
            "--session",
            "gain-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 100); do echo line-$i; done",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());

    let gain = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "gain",
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "gain-session",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(gain.status.success());
    let json: serde_json::Value = serde_json::from_slice(&gain.stdout).unwrap();
    assert_eq!(json["commands"], 1);
    assert!(json["raw_bytes"].as_u64().unwrap() > 0);
    assert!(json["model_bytes"].as_u64().unwrap() > 0);
}

#[test]
fn setup_codex_apply_respects_global_scope_and_uninstall() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let project_agents = dir.path().join("AGENTS.md");
    let global_agents = home.path().join(".codex").join("AGENTS.md");

    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .args(["setup", "--ai", "--codex", "--apply", "--global"])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(!project_agents.exists());
    let text = std::fs::read_to_string(&global_agents).unwrap();
    assert_eq!(text.matches("TFY:CODEX:START").count(), 1, "{text}");

    let uninstall = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("HOME", home.path())
        .args([
            "setup",
            "--ai",
            "--codex",
            "--uninstall",
            "--apply",
            "--global",
        ])
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let text = std::fs::read_to_string(&global_agents).unwrap();
    assert!(!text.contains("TFY:CODEX:"), "{text}");
}

#[test]
fn setup_status_and_explain_make_supported_boundaries_obvious() {
    let dir = tempfile::tempdir().unwrap();
    let setup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["setup", "--ai", "--codex", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let text = String::from_utf8_lossy(&setup.stdout);
    assert!(text.contains("supported_host_routing=true"), "{text}");
    assert!(
        text.contains("ordinary_terminal_interception=false"),
        "{text}"
    );
    assert!(text.contains("provider_gateway=false"), "{text}");
    assert!(text.contains("editor_integration=false"), "{text}");
    assert!(!dir.path().join("AGENTS.md").exists());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["status"], "active");
    let surfaces = json["surfaces"].as_array().unwrap();
    assert!(surfaces
        .iter()
        .any(|s| s["name"] == "compact_code_restore" && s["status"] == "active"));
    assert!(surfaces
        .iter()
        .any(|s| s["name"] == "provider_api_gateway" && s["status"] == "not_supported"));
    assert!(surfaces
        .iter()
        .any(|s| s["name"] == "editor_integration" && s["status"] == "not_supported"));
    assert!(surfaces
        .iter()
        .any(|s| s["name"] == "private_codex_hook" && s["status"] == "not_supported"));

    let explain = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["explain"])
        .output()
        .unwrap();
    assert!(explain.status.success());
    let explain_text = String::from_utf8_lossy(&explain.stdout);
    assert!(explain_text.contains("function f0(a,b)"), "{explain_text}");
    assert!(
        explain_text.contains("restores original/readable names"),
        "{explain_text}"
    );
    assert!(
        explain_text.contains("provider/API gateway proxy"),
        "{explain_text}"
    );
}

#[test]
fn repeated_adapter_output_is_elided_only_after_raw_evidence_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let noisy = "for i in $(seq 1 120); do echo repeated-line-$i; done";

    let first = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "adapter",
            "run",
            "--session",
            "repeat-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            noisy,
        ])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first_text = String::from_utf8_lossy(&first.stdout);
    assert!(!first_text.contains("repeat_elided"), "{first_text}");

    let second = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "adapter",
            "run",
            "--session",
            "repeat-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            noisy,
        ])
        .output()
        .unwrap();
    assert!(second.status.success());
    let second_text = String::from_utf8_lossy(&second.stdout);
    assert!(
        second_text.contains("repeated unchanged command output elided"),
        "{second_text}"
    );
    assert!(second_text.contains("previous_raw_ref="), "{second_text}");
    assert!(second_text.contains("raw_ref="), "{second_text}");
    assert!(std::fs::read_dir(&raw).unwrap().count() >= 2);

    let gain = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "gain",
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "repeat-session",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(gain.status.success());
    let json: serde_json::Value = serde_json::from_slice(&gain.stdout).unwrap();
    assert_eq!(json["rendering_counts"]["repeat_elided"], 1);
    assert!(json["saved_bytes"].as_i64().unwrap() > 0, "{json}");
}

#[test]
fn repeated_elision_uses_exact_raw_bytes_not_lossy_text() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let script = "python3 -c 'import os,sys; sys.stdout.buffer.write(bytes([int(os.environ[\"BYTE\"])]) * 240)'";

    let first = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("BYTE", "255")
        .args([
            "adapter",
            "run",
            "--session",
            "binary-repeat-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            script,
        ])
        .output()
        .unwrap();
    assert!(first.status.success());

    let second = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("BYTE", "254")
        .args([
            "adapter",
            "run",
            "--session",
            "binary-repeat-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            script,
        ])
        .output()
        .unwrap();
    assert!(second.status.success());
    let second_text = String::from_utf8_lossy(&second.stdout);
    assert!(
        !second_text.contains("repeated unchanged command output elided"),
        "{second_text}"
    );
    assert!(
        second_text.contains("output suppressed") || !second_text.contains("repeat_elided"),
        "{second_text}"
    );

    let gain = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "gain",
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "binary-repeat-session",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(gain.status.success());
    let json: serde_json::Value = serde_json::from_slice(&gain.stdout).unwrap();
    assert!(
        json["rendering_counts"].get("repeat_elided").is_none(),
        "{json}"
    );
}

#[test]
fn launch_report_promotes_adapter_when_gain_ledger_has_command_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let run = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "adapter",
            "run",
            "--session",
            "launch-evidence",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 100); do echo launch-evidence-$i; done",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "launch-evidence",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let adapter = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "generic_shell")
        .unwrap();
    assert_eq!(adapter["status"], "verified_local_mcp");
    assert!(adapter["launch_claim"]
        .as_str()
        .unwrap()
        .contains("local smoke verified"));
    assert_eq!(json["status"], "blocked");
    assert!(json["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| { b.as_str().unwrap().contains("required v1 host mcp_stdio") }));
}

#[test]
fn launch_report_blocks_unverified_codex_and_exposes_v1_host_matrix() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["launch-report", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "blocked");
    assert!(json["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| { b.as_str().unwrap().contains("required v1 host mcp_stdio") }));
    let hosts = json["host_matrix"].as_array().unwrap();
    assert!(hosts.iter().any(|h| h["host"] == "mcp_stdio"));
    assert!(hosts.iter().any(|h| h["host"] == "tfy_agent_adapter"));
    assert!(hosts.iter().any(|h| h["host"] == "generic_shell"));
    assert!(hosts.iter().all(|h| h.get("normal_workflow").is_some()));
    assert!(hosts
        .iter()
        .filter(|h| ["mcp_stdio", "tfy_agent_adapter", "generic_shell"]
            .contains(&h["host"].as_str().unwrap()))
        .all(|h| h["required_for_v1"] == true));
    assert!(hosts
        .iter()
        .any(|h| h["host"] == "codex" && h["status"] == "config_snippet_available"));
    assert!(json["not_supported"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "provider_api_gateway"));
    assert!(
        json["required_benchmark_scenarios"]
            .as_array()
            .unwrap()
            .len()
            >= 7
    );
    assert_eq!(json["measurement_method"]["tokenizer_exact"], false);
    assert_eq!(json["unsupported_claim_audit"]["status"], "pass");
}

#[test]
fn launch_report_blocks_local_smoke_without_host_evidence_and_keeps_codex_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let evidence = smoke_json["evidence"].as_array().unwrap();
    let mut ledgers = Vec::new();
    for entry in evidence {
        let text = entry.as_str().unwrap();
        if let Some(path) = text.strip_prefix("ledger=") {
            ledgers.push(path.to_string());
        }
    }
    assert!(ledgers.len() >= 2, "{smoke_json}");
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for ledger in &ledgers {
        owned_args.push("--ledger".into());
        owned_args.push(ledger.clone());
    }
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["status"], "blocked", "{json}");
    let hosts = json["host_matrix"].as_array().unwrap();
    for required in ["mcp_stdio", "tfy_agent_adapter", "generic_shell"] {
        assert!(
            hosts
                .iter()
                .any(|h| h["host"] == required && h["status"] == "verified_local_mcp"),
            "{json}"
        );
    }
    assert!(hosts
        .iter()
        .any(|h| h["host"] == "codex" && h["status"] == "config_snippet_available"));
    assert_eq!(json["host_evidence"]["mcp_state"], true);
    assert_eq!(json["host_evidence"]["positive_savings"], true);
}

#[test]
fn launch_report_passes_only_with_route_smoke_and_host_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    append_smoke_launch_args(&mut owned_args, &smoke_json);
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    let hosts = json["host_matrix"].as_array().unwrap();
    for required in ["mcp_stdio", "tfy_agent_adapter", "generic_shell"] {
        let host = hosts.iter().find(|h| h["host"] == required).unwrap();
        assert_eq!(host["status"], "launch_supported", "{json}");
        let gates = host["evidence_gate"].as_array().unwrap();
        assert!(
            gates.iter().any(|gate| gate
                .as_str()
                .unwrap()
                .contains("observed route_commands=1 route_raw_refs=1")),
            "{json}"
        );
        assert!(
            !gates
                .iter()
                .any(|gate| gate.as_str().unwrap().contains("saved_bytes=")),
            "{json}"
        );
    }
    assert!(hosts
        .iter()
        .any(|h| h["host"] == "codex" && h["status"] == "config_snippet_available"));
}

#[test]
fn launch_report_rejects_boolean_only_host_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for entry in smoke_json["evidence"].as_array().unwrap() {
        let text = entry.as_str().unwrap();
        if let Some(path) = text.strip_prefix("ledger=") {
            owned_args.push("--ledger".into());
            owned_args.push(path.to_string());
        }
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {"host":"generic_shell","setup_verified":true,"real_invocation_verified":true},
            {"host":"tfy_agent_adapter","setup_verified":true,"real_invocation_verified":true},
            {"host":"mcp_stdio","setup_verified":true,"real_invocation_verified":true}
          ]
        }"#,
    )
    .unwrap();
    owned_args.push("--host-evidence".into());
    owned_args.push(host_evidence.display().to_string());
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["status"], "blocked", "{json}");
    let hosts = json["host_matrix"].as_array().unwrap();
    for required in ["mcp_stdio", "tfy_agent_adapter", "generic_shell"] {
        assert!(
            hosts
                .iter()
                .any(|h| h["host"] == required && h["status"] == "verified_local_mcp"),
            "{json}"
        );
    }
}

#[test]
fn launch_report_classifies_agent_route_without_ledger_name_hint() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("route.jsonl");
    let raw = dir.path().join("raw");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "agent",
            "run",
            "--session",
            "agent-s",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo tfy-agent-route-$i; done",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--session",
            "agent-s",
            "--ledger",
            ledger.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let hosts = json["host_matrix"].as_array().unwrap();
    assert!(
        hosts
            .iter()
            .any(|h| h["host"] == "tfy_agent_adapter" && h["status"] == "verified_local_mcp"),
        "{json}"
    );
    assert_eq!(json["host_evidence"]["tfy_agent_adapter"], true);
    assert_eq!(json["host_evidence"]["generic_shell_wrapper"], false);
}

#[test]
fn launch_report_accepts_artifact_backed_overhead_exception() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    append_smoke_launch_args(&mut owned_args, &smoke_json);
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    let hosts = json["host_matrix"].as_array().unwrap();
    for required in ["mcp_stdio", "tfy_agent_adapter", "generic_shell"] {
        let host = hosts.iter().find(|h| h["host"] == required).unwrap();
        assert_eq!(host["status"], "launch_supported", "{json}");
        assert!(
            host["evidence_gate"]
                .as_array()
                .unwrap()
                .iter()
                .any(|gate| gate
                    .as_str()
                    .unwrap()
                    .contains("observed route_overhead_exception=")),
            "{json}"
        );
    }
}

#[test]
fn launch_report_promotes_claims_only_from_route_bound_evidence_tiers() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "cursor-config.json",
        "cursor-ledger.jsonl",
        "cursor-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "host_id":"cursor",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"cursor-config.json",
              "route_type":"mcp_stdio",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"cursor-raw.txt",
              "smoke_id":"cursor-mcp-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "overhead_ms":10,
              "baseline_ms":10
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "cursor")
        .unwrap();
    assert_eq!(cursor["status"], "launch_supported", "{json}");
    for tier in [
        "config_written",
        "host_launched",
        "verified_host_mcp_invocation",
        "route_evidence_recorded",
        "savings_verified",
        "launch_supported",
    ] {
        assert!(
            cursor["evidence_tiers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|observed| observed == tier),
            "missing {tier}: {json}"
        );
    }
    assert_eq!(cursor["next_evidence_tier"], "complete", "{json}");
    assert!(json["unsupported_claim_audit"]["rule"]
        .as_str()
        .unwrap()
        .contains("private Codex hook interception"));
    assert!(json["not_supported"]
        .as_array()
        .unwrap()
        .iter()
        .any(|surface| surface == "provider_api_prompt_proxy"));
}

#[test]
fn launch_report_refuses_named_host_unsupported_route_type_even_with_artifacts_and_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "cursor-config.json",
        "cursor-ledger.jsonl",
        "cursor-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "host_id":"cursor",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"cursor-config.json",
              "route_type":"provider_api_prompt_proxy",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"cursor-raw.txt",
              "smoke_id":"cursor-provider-proxy-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "overhead_ms":10,
              "baseline_ms":10
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert!(!cursor["evidence_tiers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tier| tier == "route_evidence_recorded"));
    assert!(json["not_supported"]
        .as_array()
        .unwrap()
        .iter()
        .any(|surface| surface == "provider_api_prompt_proxy"));
}

#[test]
fn launch_report_refuses_official_hook_promotion_without_supported_hook_authority() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "cursor-config.json",
        "cursor-ledger.jsonl",
        "cursor-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.1.0",
              "host_id":"cursor",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"cursor-config.json",
              "route_type":"official_host_hook",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"cursor-raw.txt",
              "smoke_id":"cursor-hook-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "official_docs_backed":true,
              "kill_switch_available":true,
              "uninstall_available":true,
              "overhead_ms":10,
              "baseline_ms":10
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert!(!cursor["evidence_tiers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tier| tier == "verified_host_hook"));
    assert!(json["host_evidence"]["evidence_notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|note| note.as_str().unwrap().contains("hook_authorized=false")));
}

#[test]
fn raw_lifecycle_lists_inspects_exports_and_prunes_with_dry_run_gate() {
    let dir = tempfile::tempdir().unwrap();
    let raw_dir = dir.path().join("raw");
    let ledger = dir.path().join("ledger.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "tool-gateway",
            "--json",
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--ledger",
            ledger.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 40); do echo raw-life-$i; done",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let gateway_json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let raw_ref = gateway_json["provenance"]["raw_refs"][0]
        .as_str()
        .unwrap()
        .to_string();

    let list = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "raw",
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--list",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(list_json["count"], 1);
    assert_eq!(list_json["entries"][0]["raw_ref"], raw_ref);
    assert_eq!(list_json["entries"][0]["command_present"], true);
    assert!(list_json["entries"][0]["command"].is_null(), "{list_json}");
    assert!(
        list_json["entries"][0]["command_sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64),
        "{list_json}"
    );

    let inspect = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "raw",
            &raw_ref,
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--inspect",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(inspect.status.success());
    let inspect_json: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(inspect_json["raw_ref"], raw_ref);
    assert!(inspect_json["bytes"].as_u64().unwrap() > 0);
    assert!(inspect_json["command"].is_null(), "{inspect_json}");

    let export_dir = dir.path().join("exported");
    let export = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "raw",
            &raw_ref,
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--export",
            export_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(export.status.success());
    assert!(export_dir.join(format!("{raw_ref}.json")).is_file());

    let dry_prune = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "raw",
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--prune",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(dry_prune.status.success());
    let dry_json: serde_json::Value = serde_json::from_slice(&dry_prune.stdout).unwrap();
    assert_eq!(dry_json["dry_run"], true);
    assert!(raw_dir.join(format!("{raw_ref}.json")).is_file());

    let apply_prune = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "raw",
            "--raw-dir",
            raw_dir.to_str().unwrap(),
            "--prune",
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(apply_prune.status.success());
    let apply_json: serde_json::Value = serde_json::from_slice(&apply_prune.stdout).unwrap();
    assert_eq!(apply_json["applied"], true);
    assert!(!raw_dir.join(format!("{raw_ref}.json")).exists());
}

#[test]
fn bench_manifest_is_self_benchmark_and_fails_closed_for_public_rtk_claim() {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("bench-manifest.json");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "bench",
            "--json",
            "--output",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    assert_eq!(json["tfy_self_benchmark"]["no_negative_savings"], true);
    assert_eq!(json["tfy_self_benchmark"]["positive_savings"], true);
    assert_eq!(json["rtk_comparator"]["status"], "skipped_unavailable");
    assert_eq!(json["public_superiority_claim_ready"], false);
    assert!(json["claim_policy"]
        .as_str()
        .unwrap()
        .contains("fails closed"));
    assert!(manifest_path.is_file());
}

#[test]
fn launch_report_release_tiers_require_release_evidence_and_keep_ga_blocked_without_named_host() {
    let dir = tempfile::tempdir().unwrap();
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["smoke", "--all", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let smoke_json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    append_smoke_launch_args(&mut owned_args, &smoke_json);
    let release_evidence = write_release_evidence_fixture(&dir);
    owned_args.push("--release-evidence".into());
    owned_args.push(release_evidence.display().to_string());
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(&owned_args)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    assert_eq!(
        json["release_tiers"]["developer_preview_ready"]["status"], "ready",
        "{json}"
    );
    assert_eq!(
        json["release_tiers"]["rc_ready"]["status"], "ready",
        "{json}"
    );
    assert_eq!(
        json["release_tiers"]["ga_ready"]["status"], "blocked",
        "{json}"
    );
    assert!(json["release_tiers"]["ga_ready"]["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|blocker| blocker.as_str().unwrap().contains("no named AI host")));
    assert_eq!(
        json["release_tiers"]["public_superiority_claim_ready"]["status"],
        "blocked"
    );
}

#[test]
fn lifecycle_help_preserves_existing_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in [
        "start",
        "stop",
        "fuckyou",
        "global",
        "raw",
        "setup",
        "init",
        "mcp",
        "adapter",
        "agent",
        "tool-gateway",
        "context-gateway",
        "output-gateway",
        "workspace",
    ] {
        assert!(text.contains(command), "missing {command} in {text}");
    }

    for args in [
        vec!["start", "--help"],
        vec!["stop", "--help"],
        vec!["fuckyou", "--help"],
        vec!["global", "--help"],
        vec!["global", "start", "--help"],
        vec!["raw", "--help"],
        vec!["tool-gateway", "--help"],
        vec!["context-gateway", "--help"],
        vec!["output-gateway", "--help"],
        vec!["workspace", "--help"],
    ] {
        let help = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .args(args)
            .output()
            .unwrap();
        assert!(help.status.success());
    }
}

#[test]
fn lifecycle_project_start_stop_restart_status_truthful() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--human"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let start_text = String::from_utf8_lossy(&start.stdout);
    assert!(
        start_text.contains("host_route_configured_verification_required"),
        "{start_text}"
    );
    assert!(
        start_text.contains("ordinary_terminal_interception=false"),
        "{start_text}"
    );
    assert!(
        start_text.contains("ordinary terminal commands are not globally intercepted"),
        "{start_text}"
    );
    assert!(dir.path().join(".tfy/lifecycle.json").exists());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["configured"], true);
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], true);
    assert_eq!(
        json["project_lifecycle"]["agent"]["support_status"],
        "host_route_configured_verification_required"
    );
    assert!(json["project_lifecycle"]["agent"]["active_routes"].is_null());
    assert_eq!(
        json["project_lifecycle"]["agent"]["intended_routes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["private_hook_interception"],
        false
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["provider_prompt_gateway"],
        false
    );
    assert_eq!(json["project_lifecycle"]["human"]["configured"], true);
    assert_eq!(json["project_lifecycle"]["human"]["desired"], true);
    assert_eq!(
        json["project_lifecycle"]["human"]["ordinary_terminal_interception"],
        false
    );
    let human_support_status = if cfg!(target_os = "linux") {
        "managed_session_available"
    } else {
        "managed_session_unsupported_platform"
    };
    assert_eq!(
        json["project_lifecycle"]["human"]["support_status"],
        human_support_status
    );
    assert!(json["minimum_v1_host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["host"] == "codex" && h["status"] != "launch_supported"));

    let stop = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["stop", "--agent"])
        .output()
        .unwrap();
    assert!(stop.status.success());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["target_filter"], "agent");
    assert_eq!(json["project_lifecycle"]["agent"]["configured"], true);
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], false);
    assert!(json["project_lifecycle"]["agent"]["stopped_at"]
        .as_str()
        .unwrap()
        .starts_with("unix:"));
    assert!(json["project_lifecycle"]["human"].is_null());

    let restart = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(restart.status.success());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], true);
    assert!(json["project_lifecycle"]["agent"]["stopped_at"].is_null());
}

#[test]
fn lifecycle_fuckyou_is_scoped_confirmed_and_preserves_raw_shared_ledgers() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    for sub in ["raw", "mcp", "adapter", "state", "agent", "human"] {
        std::fs::create_dir_all(tfy.join(sub)).unwrap();
        std::fs::write(tfy.join(sub).join("keep.txt"), sub).unwrap();
    }
    std::fs::write(tfy.join("agent/ledger.jsonl"), "agent evidence").unwrap();
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--human"])
        .output()
        .unwrap();

    let blocked = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent"])
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(tfy.join("agent/keep.txt").exists());

    let cleaned = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(
        cleaned.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    assert!(!tfy.join("agent/keep.txt").exists());
    assert!(tfy.join("agent/ledger.jsonl").exists());
    for sub in ["raw", "mcp", "adapter", "state", "human"] {
        assert!(
            tfy.join(sub).join("keep.txt").exists(),
            "{sub} should remain"
        );
    }
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(json["project_lifecycle"]["agent"].is_null());
    assert_eq!(json["project_lifecycle"]["human"]["configured"], true);
}

#[test]
fn lifecycle_global_uses_tfy_home_and_stays_separate_from_project() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["global", "start", "--agent", "--human"])
        .output()
        .unwrap();
    assert!(start.status.success());
    assert!(home.path().join("lifecycle.json").exists());
    assert!(home.path().join("raw").is_dir());
    assert!(home.path().join("state").is_dir());
    assert!(home.path().join("adapter").is_dir());
    assert!(home.path().join("agent").is_dir());
    assert!(home.path().join("mcp").is_dir());
    assert!(!dir.path().join(".tfy/lifecycle.json").exists());

    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], true);
    assert_eq!(
        json["global_lifecycle"]["human"]["ordinary_terminal_interception"],
        false
    );

    let cleanup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["global", "fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(cleanup.status.success());
    assert!(dir.path().join(".tfy/lifecycle.json").exists());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(json["global_lifecycle"]["agent"].is_null());
    assert_eq!(json["global_lifecycle"]["human"]["desired"], true);
}

#[test]
fn lifecycle_bare_fuckyou_reads_target_and_confirmation_from_one_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(tfy.join("raw")).unwrap();
    std::fs::create_dir_all(tfy.join("human")).unwrap();
    std::fs::write(tfy.join("raw/keep.txt"), "raw").unwrap();
    std::fs::write(tfy.join("human/remove.txt"), "human").unwrap();

    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--human"])
        .output()
        .unwrap();
    assert!(start.status.success());

    let mut child = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .arg("fuckyou")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"human\nyes\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!tfy.join("human").exists());
    assert!(tfy.join("raw/keep.txt").exists());
    assert!(!tfy.join("lifecycle.json").exists());
}

#[test]
fn lifecycle_status_and_stop_accept_legacy_active_routes_state() {
    let dir = tempfile::tempdir().unwrap();
    let lifecycle = dir.path().join(".tfy/lifecycle.json");
    std::fs::create_dir_all(lifecycle.parent().unwrap()).unwrap();
    std::fs::write(
        &lifecycle,
        r#"{
  "schema_version": 1,
  "scope": "project",
  "agent": {
    "configured": true,
    "desired": true,
    "active_routes": ["mcp_stdio"],
    "private_hook_interception": false,
    "provider_prompt_gateway": false,
    "support_status": "host_route_configuration_required",
    "started_at": "unix:1",
    "stopped_at": null
  },
  "human": null,
  "raw_dir": ".tfy/raw",
  "ledgers": {
    "state": ".tfy/state/ledger.jsonl",
    "adapter": ".tfy/adapter/ledger.jsonl",
    "agent": ".tfy/agent/ledger.jsonl",
    "mcp": ".tfy/mcp/ledger.jsonl"
  }
}
"#,
    )
    .unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&status.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(json["project_lifecycle"]["parse_error"].is_null());
    assert_eq!(json["project_lifecycle"]["agent"]["configured"], true);
    assert!(json["project_lifecycle"]["agent"]["active_routes"].is_null());
    assert_eq!(
        json["project_lifecycle"]["agent"]["intended_routes"][0],
        "mcp_stdio"
    );

    let stop = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["stop", "--agent"])
        .output()
        .unwrap();
    assert!(
        stop.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&stop.stderr)
    );
    let migrated: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&lifecycle).unwrap()).unwrap();
    assert!(migrated["agent"]["active_routes"].is_null());
    assert_eq!(migrated["agent"]["intended_routes"][0], "mcp_stdio");
    assert_eq!(migrated["agent"]["started_at"], "unix:1");
    assert_eq!(migrated["agent"]["desired"], false);
}

#[test]
fn lifecycle_stop_prevents_active_claim_even_with_retained_launch_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let lifecycle = dir.path().join(".tfy/lifecycle.json");
    std::fs::create_dir_all(lifecycle.parent().unwrap()).unwrap();
    std::fs::write(
        &lifecycle,
        r#"{
  "schema_version": 1,
  "scope": "project",
  "agent": {
    "configured": true,
    "desired": false,
    "route_state": "intent_recorded",
    "active": false,
    "intended_routes": ["mcp_stdio", "tfy_agent_adapter", "generic_shell"],
    "host_routes": {
      "codex": {
        "route_state": "launch_supported",
        "active": false,
        "configured": true,
        "route_configured": true,
        "host_reload_required": false,
        "host_route_available_after_reload": true,
        "host_approval_required": false,
        "mcp_invocation_observed": true,
        "route_verified": true,
        "savings_verified": true,
        "normal_workflow_supported": true,
        "config_path": ".codex/config.toml",
        "claim_tier": "launch_supported",
        "message": "retained historical evidence after stop"
      }
    },
    "private_hook_interception": false,
    "provider_prompt_gateway": false,
    "support_status": "host_route_configured_verification_required",
    "started_at": "unix:1",
    "stopped_at": "unix:2"
  },
  "human": null,
  "raw_dir": ".tfy/raw",
  "ledgers": {
    "state": ".tfy/state/ledger.jsonl",
    "adapter": ".tfy/adapter/ledger.jsonl",
    "agent": ".tfy/agent/ledger.jsonl",
    "mcp": ".tfy/mcp/ledger.jsonl"
  }
}
"#,
    )
    .unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&status.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["effective_lifecycle"]["agent"]["desired"], false);
    assert_eq!(json["effective_lifecycle"]["agent"]["route_verified"], true);
    assert_eq!(
        json["effective_lifecycle"]["agent"]["savings_verified"],
        true
    );
    assert_eq!(
        json["effective_lifecycle"]["agent"]["normal_workflow_supported"],
        true
    );
    assert_eq!(json["effective_lifecycle"]["agent"]["active"], false);
    assert_eq!(json["lifecycle_summary"]["active"], false);
    assert_ne!(json["lifecycle_summary"]["status"], "active");
    assert!(json["effective_lifecycle"]["agent"]["active_derivation"]
        .as_str()
        .unwrap()
        .contains("lifecycle desire is on"));
}

#[test]
fn lifecycle_status_exposes_malformed_state_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let lifecycle = dir.path().join(".tfy/lifecycle.json");
    std::fs::create_dir_all(lifecycle.parent().unwrap()).unwrap();
    std::fs::write(&lifecycle, "not-json").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["exists"], true);
    assert!(json["project_lifecycle"]["parse_error"]
        .as_str()
        .unwrap()
        .contains("parse .tfy/lifecycle.json"));
    assert_eq!(
        json["effective_lifecycle"]["agent"]["desired_source"],
        "project_parse_error"
    );
    assert_eq!(
        json["effective_lifecycle"]["agent"]["route_state"],
        "lifecycle_parse_error"
    );
}

#[test]
fn lifecycle_status_filters_effective_target_scope() {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(json["project_lifecycle"]["human"].is_null());
    assert!(json["effective_lifecycle"]["human"].is_null());
    assert_eq!(
        json["effective_lifecycle"]["agent"]["desired_source"],
        "none"
    );

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--human", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(json["project_lifecycle"]["agent"].is_null());
    assert!(json["effective_lifecycle"]["agent"].is_null());
    assert_eq!(
        json["effective_lifecycle"]["human"]["desired_source"],
        "none"
    );
}

#[test]
fn lifecycle_stop_without_prior_start_does_not_fabricate_started_at() {
    let dir = tempfile::tempdir().unwrap();
    let stop = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["stop", "--agent", "--human"])
        .output()
        .unwrap();
    assert!(stop.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["configured"], true);
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], false);
    assert!(json["project_lifecycle"]["agent"]["started_at"].is_null());
    assert!(json["project_lifecycle"]["agent"]["stopped_at"]
        .as_str()
        .unwrap()
        .starts_with("unix:"));
    assert_eq!(json["project_lifecycle"]["human"]["configured"], true);
    assert_eq!(json["project_lifecycle"]["human"]["desired"], false);
    assert!(json["project_lifecycle"]["human"]["started_at"].is_null());
    assert!(json["project_lifecycle"]["human"]["stopped_at"]
        .as_str()
        .unwrap()
        .starts_with("unix:"));
}

#[test]
fn lifecycle_global_start_accepts_host_options_as_default_guidance_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["global", "start", "--agent", "--host", "all", "--apply"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let text = String::from_utf8_lossy(&start.stdout);
    assert!(text.contains("global_host_apply=false"), "{text}");
    assert!(
        text.contains("project_scoped_host_config_required"),
        "{text}"
    );
    assert!(!dir.path().join(".codex/config.toml").exists());
    assert!(!dir.path().join(".mcp.json").exists());
    assert!(!dir.path().join(".cursor/mcp.json").exists());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["global_lifecycle"]["agent"]["desired"], true);
    assert_eq!(
        json["global_lifecycle"]["agent"]["host_routes"]["codex"]["route_state"],
        "config_snippet_available"
    );
    assert_eq!(
        json["global_lifecycle"]["agent"]["host_routes"]["claude-code"]["route_state"],
        "config_snippet_available"
    );
    assert_eq!(
        json["global_lifecycle"]["agent"]["host_routes"]["cursor"]["route_state"],
        "config_snippet_available"
    );
    assert_eq!(json["effective_lifecycle"]["agent"]["active"], false);
    assert_eq!(json["lifecycle_summary"]["status"], "intent_recorded");
}

#[test]
fn lifecycle_bare_commands_fail_closed_without_target_choice() {
    for command in ["start", "stop", "fuckyou"] {
        let dir = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .current_dir(dir.path())
            .arg(command)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{command} should fail closed");
        assert!(!dir.path().join(".tfy/lifecycle.json").exists());
    }
}

#[test]
fn lifecycle_help_advertises_bare_tui_wizard() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("target TUI wizard"), "{text}");
    assert!(text.contains("confirmation TUI wizards"), "{text}");
}

#[test]
fn lifecycle_bare_commands_accept_explicit_stdin_choices() {
    let dir = tempfile::tempdir().unwrap();
    let mut start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .arg("start")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    start.stdin.as_mut().unwrap().write_all(b"both\n").unwrap();
    let output = start.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut stop = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .arg("stop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    stop.stdin.as_mut().unwrap().write_all(b"human\n").unwrap();
    let output = stop.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], true);
    assert_eq!(json["project_lifecycle"]["human"]["desired"], false);
}

#[test]
fn lifecycle_status_reports_effective_state_and_next_actions() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let fresh_status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(fresh_status.status.success());
    let fresh_json: serde_json::Value = serde_json::from_slice(&fresh_status.stdout).unwrap();
    assert_eq!(
        fresh_json["effective_lifecycle"]["agent"]["desired_source"],
        "none"
    );
    assert_eq!(fresh_json["effective_lifecycle"]["agent"]["desired"], false);
    assert_eq!(
        fresh_json["effective_lifecycle"]["agent"]["configured"],
        false
    );
    assert_eq!(fresh_json["effective_lifecycle"]["agent"]["active"], false);
    assert_eq!(fresh_json["lifecycle_summary"]["status"], "not_configured");
    assert_eq!(fresh_json["lifecycle_summary"]["active"], false);
    assert_eq!(fresh_json["lifecycle_summary"]["evidence_required"], false);

    let global_start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["global", "start", "agent"])
        .output()
        .unwrap();
    assert!(global_start.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["effective_lifecycle"]["agent"]["desired"], true);
    assert_eq!(
        json["effective_lifecycle"]["agent"]["desired_source"],
        "global"
    );
    assert_eq!(json["effective_lifecycle"]["agent"]["active"], false);
    assert_eq!(json["lifecycle_summary"]["status"], "intent_recorded");
    assert_eq!(json["lifecycle_summary"]["desired"], true);
    assert_eq!(json["lifecycle_summary"]["active"], false);
    assert_eq!(json["lifecycle_summary"]["evidence_required"], true);
    assert!(json["effective_lifecycle"]["agent"]["next_action"]
        .as_str()
        .unwrap()
        .contains("tfy start --agent"));

    let project_start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["start", "agent", "--host", "cursor", "--apply"])
        .output()
        .unwrap();
    assert!(project_start.status.success());
    let text_status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .arg("status")
        .output()
        .unwrap();
    assert!(text_status.status.success());
    let text = String::from_utf8_lossy(&text_status.stdout);
    assert!(
        text.contains("effective agent: desired=true source=project"),
        "{text}"
    );
    assert!(
        text.contains("lifecycle status: configured_unverified"),
        "{text}"
    );
    assert!(text.contains("next agent action:"), "{text}");
    assert!(text.contains("active=false"), "{text}");
}

#[test]
fn lifecycle_use_always_alias_controls_global_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["use", "always", "--agent"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );

    let typo_alias = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["use", "alwais", "--human"])
        .output()
        .unwrap();
    assert!(typo_alias.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["global_lifecycle"]["agent"]["desired"], true);
    assert_eq!(json["global_lifecycle"]["human"]["desired"], true);
    assert_eq!(
        json["lifecycle_summary"]["status"],
        if cfg!(target_os = "linux") {
            "configured_unverified"
        } else {
            "intent_recorded"
        }
    );
    assert_eq!(
        json["lifecycle_summary"]["desired_targets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let cancel = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["use", "cancel", "--agent"])
        .output()
        .unwrap();
    assert!(cancel.status.success());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .env("TFY_HOME", home.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["global_lifecycle"]["agent"]["desired"], false);
    assert_eq!(json["lifecycle_summary"]["status"], "intent_recorded");
    assert_eq!(json["lifecycle_summary"]["desired"], false);
    assert_eq!(json["lifecycle_summary"]["configured"], true);
}

#[test]
fn lifecycle_start_verify_prints_persisted_support_status() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "agent", "--host", "cursor", "--verify"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let stdout = String::from_utf8_lossy(&start.stdout);
    assert!(
        stdout.contains("support_status=verification_requested_route_evidence_required"),
        "{stdout}"
    );
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        json["project_lifecycle"]["agent"]["support_status"],
        "verification_requested_route_evidence_required"
    );
}

#[test]
fn lifecycle_target_aliases_match_flag_targets() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "ai"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "human"])
        .output()
        .unwrap();
    assert!(start.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], true);
    assert_eq!(json["project_lifecycle"]["human"]["desired"], true);
    assert_eq!(
        json["project_lifecycle"]["agent"]["route_state"],
        "intent_recorded"
    );
    assert_eq!(json["project_lifecycle"]["agent"]["active"], false);
    assert_eq!(
        json["project_lifecycle"]["human"]["route_state"],
        "intent_recorded"
    );
    assert_eq!(json["project_lifecycle"]["human"]["active"], false);

    let stop = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["stop", "both"])
        .output()
        .unwrap();
    assert!(stop.status.success());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(json["project_lifecycle"]["agent"]["desired"], false);
    assert_eq!(json["project_lifecycle"]["human"]["desired"], false);
}

#[test]
fn lifecycle_start_cursor_apply_caps_at_configured_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "agent", "--host", "cursor", "--apply"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let text = String::from_utf8_lossy(&start.stdout);
    assert!(text.contains("route_state=configured_unverified"), "{text}");
    assert!(
        text.contains("Configured Cursor project MCP route: .cursor/mcp.json"),
        "{text}"
    );
    assert!(
        text.contains("Restart/reload Cursor to make the route visible."),
        "{text}"
    );
    assert!(text.contains("Status: configured, not active."), "{text}");
    assert!(
        text.contains("Next: open Cursor and invoke a TFY MCP tool to verify real host routing."),
        "{text}"
    );
    assert!(!text.contains("Cursor is now saving tokens"), "{text}");
    assert!(text.contains("active=false"), "{text}");
    assert!(dir.path().join(".cursor/mcp.json").exists());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let cursor = &json["project_lifecycle"]["agent"]["host_routes"]["cursor"];
    assert_eq!(cursor["route_state"], "configured_unverified");
    assert_eq!(cursor["active"], false);
    assert_eq!(cursor["route_configured"], true);
    assert_eq!(cursor["host_reload_required"], true);
    assert_eq!(cursor["host_route_available_after_reload"], false);
    assert_eq!(json["project_lifecycle"]["agent"]["active"], false);
    assert!(json["minimum_v1_host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["host"] == "cursor" && h["status"] != "launch_supported"));
}

#[test]
fn lifecycle_start_host_all_apply_only_uses_safe_writers() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "agent", "--host", "all", "--apply"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let text = String::from_utf8_lossy(&start.stdout);
    assert!(
        text.contains("host=codex route_state=configured_unverified"),
        "{text}"
    );
    assert!(
        text.contains("host=claude-code route_state=configured_unverified"),
        "{text}"
    );
    assert!(
        text.contains("host=cursor route_state=configured_unverified"),
        "{text}"
    );
    assert!(text.contains("host=opencode"), "{text}");
    assert!(dir.path().join(".cursor/mcp.json").exists());
    assert!(dir.path().join(".codex/config.toml").exists());
    assert!(dir.path().join(".mcp.json").exists());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        json["project_lifecycle"]["agent"]["host_routes"]["codex"]["route_state"],
        "configured_unverified"
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["host_routes"]["claude-code"]["route_state"],
        "configured_unverified"
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["host_routes"]["cursor"]["route_configured"],
        true
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["host_routes"]["cursor"]["host_reload_required"],
        true
    );
    assert_eq!(
        json["project_lifecycle"]["agent"]["host_routes"]["claude-code"]["active"],
        false
    );
}

#[test]
fn lifecycle_start_agent_auto_configures_codex_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let text = String::from_utf8_lossy(&start.stdout);
    assert!(
        text.contains("Configured Codex project MCP route: .codex/config.toml"),
        "{text}"
    );
    assert!(
        text.contains("Open/reload Codex in a trusted project"),
        "{text}"
    );
    assert!(text.contains("Status: configured, not active."), "{text}");
    assert!(!text.contains("TFY is active"), "{text}");
    assert!(!text.contains("saving tokens"), "{text}");
    assert!(dir.path().join(".codex/config.toml").exists());
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(dir.path().join(".tfy/host-config/codex.json").exists());
    assert!(dir.path().join(".tfy/raw").is_dir());
    assert!(dir.path().join(".tfy/state").is_dir());
    assert!(dir.path().join(".tfy/adapter").is_dir());
    assert!(dir.path().join(".tfy/agent").is_dir());
    assert!(dir.path().join(".tfy/mcp").is_dir());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let codex = &json["project_lifecycle"]["agent"]["host_routes"]["codex"];
    assert_eq!(codex["route_state"], "configured_unverified", "{json}");
    assert_eq!(codex["configured"], true, "{json}");
    assert_eq!(codex["route_configured"], true, "{json}");
    assert_eq!(codex["host_reload_required"], true, "{json}");
    assert_eq!(codex["host_approval_required"], false, "{json}");
    assert_eq!(codex["host_route_available_after_reload"], false, "{json}");
    assert_eq!(codex["mcp_invocation_observed"], false, "{json}");
    assert_eq!(
        json["project_lifecycle"]["agent"]["support_status"],
        "host_route_configured_verification_required",
        "{json}"
    );
    assert_eq!(
        json["effective_lifecycle"]["agent"]["active"], false,
        "{json}"
    );
}

#[test]
fn lifecycle_start_agent_no_apply_records_intent_without_cursor_config() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--no-apply"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".codex/config.toml").exists());
    assert!(!dir.path().join(".mcp.json").exists());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        json["project_lifecycle"]["agent"]["desired"], true,
        "{json}"
    );
    assert!(
        json["project_lifecycle"]["agent"]["host_routes"]
            .as_object()
            .unwrap()
            .is_empty(),
        "{json}"
    );
    assert_eq!(
        json["effective_lifecycle"]["agent"]["route_configured"], false,
        "{json}"
    );
    assert_eq!(
        json["effective_lifecycle"]["agent"]["active"], false,
        "{json}"
    );
}

#[test]
fn lifecycle_start_agent_host_cursor_applies_without_apply_flag() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--host", "cursor"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(dir.path().join(".cursor/mcp.json").exists());
    let text = String::from_utf8_lossy(&start.stdout);
    assert!(text.contains("route_state=configured_unverified"), "{text}");
}

#[test]
fn lifecycle_start_agent_host_claude_code_applies_project_mcp_with_approval_required() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--host", "claude-code"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(dir.path().join(".mcp.json").exists());
    assert!(dir
        .path()
        .join(".tfy/host-config/claude-code.json")
        .exists());
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let claude = &json["project_lifecycle"]["agent"]["host_routes"]["claude-code"];
    assert_eq!(claude["route_state"], "configured_unverified", "{json}");
    assert_eq!(claude["configured"], true, "{json}");
    assert_eq!(claude["route_configured"], true, "{json}");
    assert_eq!(claude["host_approval_required"], true, "{json}");
    assert_eq!(claude["active"], false, "{json}");
}

#[test]
fn lifecycle_start_agent_unsupported_host_is_guidance_only_without_configured_claim() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--host", "opencode"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&start.stderr)
    );
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let opencode = &json["project_lifecycle"]["agent"]["host_routes"]["opencode"];
    assert_eq!(
        opencode["route_state"], "config_snippet_available",
        "{json}"
    );
    assert_eq!(opencode["configured"], false, "{json}");
    assert_eq!(opencode["route_configured"], false, "{json}");
    assert_eq!(opencode["active"], false, "{json}");
}

#[test]
fn lifecycle_start_agent_fails_closed_on_unowned_codex_tfy_route() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        "[mcp_servers.tfy]\ncommand = \"custom\"\n",
    )
    .unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(stderr.contains("not TFY-owned"), "{stderr}");
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_malformed_codex_toml() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(codex_dir.join("config.toml"), "[mcp_servers.tfy\n").unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_invalid_codex_toml_key_value() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "[other]
key =";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(stderr.contains("malformed TOML key/value"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_quoted_codex_tfy_route() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = r#"[mcp_servers."tfy"]
command = "custom"
"#;
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(stderr.contains("not TFY-owned"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_dotted_codex_tfy_route() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "mcp_servers.tfy.command = \"custom\"\n";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(stderr.contains("not TFY-owned"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_nested_dotted_codex_tfy_route() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "[mcp_servers]\ntfy.command = \"custom\"\n";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(stderr.contains("not TFY-owned"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_incomplete_codex_tfy_marker() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        "# keep\n[other]\nvalue = \"ok\"\n# TFY:HOST-CONFIG:START codex\n[mcp_servers.tfy]\ncommand = \"tfy\"\n[after]\nvalue = \"must stay\"\n",
    )
    .unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(
        stderr.contains("malformed TFY host config marker"),
        "{stderr}"
    );
    let codex = std::fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(codex.contains("[after]"), "{codex}");
    assert!(codex.contains("must stay"), "{codex}");
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_start_agent_fails_closed_on_orphan_codex_tfy_end_marker() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "# keep\n[other]\nvalue = \"ok\"\n# TFY:HOST-CONFIG:END codex\n[after]\nvalue = \"must stay\"\n";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    let stderr = String::from_utf8_lossy(&start.stderr);
    assert!(
        stderr.contains("malformed TFY host config marker"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn lifecycle_fuckyou_agent_fails_closed_on_incomplete_codex_tfy_marker() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        "# keep\n[other]\nvalue = \"ok\"\n# TFY:HOST-CONFIG:START codex\n[mcp_servers.tfy]\ncommand = \"tfy\"\n[after]\nvalue = \"must stay\"\n",
    )
    .unwrap();
    let cleanup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(!cleanup.status.success());
    let stderr = String::from_utf8_lossy(&cleanup.stderr);
    assert!(
        stderr.contains("malformed TFY host config marker"),
        "{stderr}"
    );
    let codex = std::fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(codex.contains("[after]"), "{codex}");
    assert!(codex.contains("must stay"), "{codex}");
}

#[test]
fn lifecycle_fuckyou_agent_fails_closed_on_orphan_codex_tfy_end_marker() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "# keep\n[other]\nvalue = \"ok\"\n# TFY:HOST-CONFIG:START codex\n[mcp_servers.tfy]\ncommand = \"tfy\"\nargs = [\"mcp\", \"serve\"]\n# TFY:HOST-CONFIG:END codex\n# TFY:HOST-CONFIG:END codex\n[after]\nvalue = \"must stay\"\n";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let cleanup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(!cleanup.status.success());
    let stderr = String::from_utf8_lossy(&cleanup.stderr);
    assert!(
        stderr.contains("malformed TFY host config marker"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
}

#[test]
fn lifecycle_fuckyou_agent_fails_closed_on_unowned_dotted_codex_tfy_route_after_owned_block() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let original = "# TFY:HOST-CONFIG:START codex\n[mcp_servers.tfy]\ncommand = \"tfy\"\nargs = [\"mcp\", \"serve\"]\n# TFY:HOST-CONFIG:END codex\nmcp_servers.tfy.command = \"custom\"\n";
    std::fs::write(codex_dir.join("config.toml"), original).unwrap();
    let cleanup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(!cleanup.status.success());
    let stderr = String::from_utf8_lossy(&cleanup.stderr);
    assert!(stderr.contains("not TFY-owned"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
        original
    );
}

#[test]
fn lifecycle_fuckyou_agent_removes_only_tfy_owned_host_entries_and_preserves_raw() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        "# keep\n[other]\nvalue = \"ok\"\n# TFY:HOST-CONFIG:START codex\n[mcp_servers.tfy]\ncommand = \"tfy\"\nargs = [\"mcp\", \"serve\"]\n# TFY:HOST-CONFIG:END codex\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".mcp.json"),
        r#"{"mcpServers":{"other":{"command":"other"},"tfy":{"command":"tfy","args":["mcp","serve"],"tfy_managed":true}},"keep":true}"#,
    )
    .unwrap();
    let cursor_dir = dir.path().join(".cursor");
    std::fs::create_dir_all(&cursor_dir).unwrap();
    let config = cursor_dir.join("mcp.json");
    std::fs::write(
        &config,
        r#"{"mcpServers":{"other":{"command":"other"},"tfy":{"command":"tfy","args":["mcp","serve"],"tfy_managed":true}},"keep":true}"#,
    )
    .unwrap();
    let raw_dir = dir.path().join(".tfy/raw");
    std::fs::create_dir_all(&raw_dir).unwrap();
    std::fs::write(raw_dir.join("raw-proof.json"), "{}").unwrap();

    let cleanup = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["fuckyou", "--agent", "--yes"])
        .output()
        .unwrap();
    assert!(
        cleanup.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&cleanup.stderr)
    );
    let codex = std::fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(codex.contains("[other]"), "{codex}");
    assert!(!codex.contains("[mcp_servers.tfy]"), "{codex}");
    let claude: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(claude["keep"], true, "{claude}");
    assert_eq!(
        claude["mcpServers"]["other"]["command"], "other",
        "{claude}"
    );
    assert!(claude["mcpServers"]["tfy"].is_null(), "{claude}");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    assert_eq!(json["keep"], true, "{json}");
    assert_eq!(json["mcpServers"]["other"]["command"], "other", "{json}");
    assert!(json["mcpServers"]["tfy"].is_null(), "{json}");
    assert!(raw_dir.join("raw-proof.json").exists());
}

#[test]
fn lifecycle_human_start_records_wrapper_metadata_without_terminal_interception() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "human"])
        .output()
        .unwrap();
    assert!(start.status.success());
    let start_text = String::from_utf8_lossy(&start.stdout);
    assert!(
        start_text.contains("ordinary_terminal_interception=false"),
        "{start_text}"
    );
    if cfg!(target_os = "linux") {
        assert!(
            start_text.contains("managed_session_launch=requires_interactive_tty"),
            "{start_text}"
        );
    } else {
        assert!(
            start_text.contains("support_status=managed_session_unsupported_platform"),
            "{start_text}"
        );
    }

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--human", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let human = &json["project_lifecycle"]["human"];
    assert_eq!(human["route_state"], "intent_recorded");
    assert_eq!(human["active"], false);
    let managed_session_available = cfg!(target_os = "linux");
    assert_eq!(
        human["session_wrapper_available"],
        managed_session_available
    );
    assert_eq!(
        human["managed_session_available"],
        managed_session_available
    );
    assert_eq!(human["managed_session_entrypoint"][0], "tfy");
    assert_eq!(human["managed_session_entrypoint"][1], "start");
    assert_eq!(human["managed_session_entrypoint"][2], "--human");
    assert_eq!(
        human["managed_session_scope"],
        "project_scoped_tfy_managed_session"
    );
    assert_eq!(human["shells_supported"][0], "linux-bash");
    assert_eq!(human["ordinary_terminal_interception"], false);
    assert_eq!(
        human["support_status"],
        if managed_session_available {
            "managed_session_available"
        } else {
            "managed_session_unsupported_platform"
        }
    );
    assert_eq!(human["entrypoint"][0], "tfy");
    assert_eq!(human["entrypoint"][1], "start");
    assert_eq!(human["entrypoint"][2], "--human");
}

#[test]
fn launch_report_demotes_stale_or_failed_reverify_named_host_evidence() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "cursor-config.json",
        "cursor-ledger.jsonl",
        "cursor-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"cursor",
              "tfy_version":"0.0.0-stale",
              "reverify_failed":true,
              "host_id":"cursor",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"cursor-config.json",
              "route_type":"mcp_stdio",
              "ledger_artifact":"cursor-ledger.jsonl",
              "raw_artifact":"cursor-raw.txt",
              "smoke_id":"cursor-mcp-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "overhead_ms":10,
              "baseline_ms":10
            }
          ]
        }"#,
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "launch-report",
            "--json",
            "--host-evidence",
            host_evidence.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let cursor = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "cursor")
        .unwrap();
    assert_ne!(cursor["status"], "launch_supported", "{json}");
    assert_eq!(
        json["host_evidence"]["named_hosts"]["cursor"]["host_bound_evidence"], false,
        "{json}"
    );
    assert!(
        json["host_evidence"]["evidence_notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note.as_str().unwrap().contains("evidence_fresh=false")),
        "{json}"
    );
}

#[test]
fn lifecycle_status_exposes_derived_activation_vocabulary() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "agent", "--host", "cursor", "--apply"])
        .output()
        .unwrap();
    assert!(start.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let agent = &json["effective_lifecycle"]["agent"];
    assert_eq!(agent["lifecycle_started"], true, "{json}");
    assert_eq!(agent["route_configured"], true, "{json}");
    assert_eq!(agent["route_verified"], false, "{json}");
    assert_eq!(agent["savings_verified"], false, "{json}");
    assert_eq!(agent["normal_workflow_supported"], false, "{json}");
    assert_eq!(agent["active"], false, "{json}");
    assert!(agent["active_derivation"]
        .as_str()
        .unwrap()
        .contains("active=false until a host+route evidence scope"));
}

#[test]
fn launch_report_accepts_unexpired_unix_evidence_and_rejects_malformed_expiry() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "cursor-config.json",
        "cursor-ledger.jsonl",
        "cursor-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let future = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    for (name, expires, should_launch) in [
        ("future.json", format!("unix:{future}"), true),
        ("malformed.json", "2026-06-12T00:00:00Z".to_string(), false),
    ] {
        let host_evidence = dir.path().join(name);
        std::fs::write(
            &host_evidence,
            format!(
                r#"{{
                  "hosts": [
                    {{
                      "host":"cursor",
                      "tfy_version":"0.1.0",
                      "evidence_expires_at":"{expires}",
                      "host_id":"cursor",
                      "host_version":"test",
                      "setup_verified":true,
                      "real_invocation_verified":true,
                      "setup_artifact":"setup-proof.txt",
                      "invocation_artifact":"invocation-proof.txt",
                      "config_scope":"project",
                      "config_path":"cursor-config.json",
                      "route_type":"mcp_stdio",
                      "ledger_artifact":"cursor-ledger.jsonl",
                      "raw_artifact":"cursor-raw.txt",
                      "smoke_id":"cursor-mcp-smoke",
                      "timestamp":"unix:{future}",
                      "redacted_public_bytes":1000,
                      "model_visible_bytes":100,
                      "overhead_ms":10,
                      "baseline_ms":10
                    }}
                  ]
                }}"#
            ),
        )
        .unwrap();
        let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .current_dir(dir.path())
            .args([
                "launch-report",
                "--json",
                "--host-evidence",
                host_evidence.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            report.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&report.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
        let cursor = json["host_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["host"] == "cursor")
            .unwrap();
        if should_launch {
            assert_eq!(cursor["status"], "launch_supported", "{json}");
        } else {
            assert_ne!(cursor["status"], "launch_supported", "{json}");
            assert!(
                json["host_evidence"]["evidence_notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|note| note
                        .as_str()
                        .unwrap()
                        .contains("evidence_expires_at_invalid_format")),
                "{json}"
            );
        }
    }
}
