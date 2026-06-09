use std::process::Command;

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
    assert!(modes.contains(&"mcp"), "{json}");
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
        r#"{"hosts":[{"host":"cursor","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup.txt","invocation_artifact":"invoke.txt","overhead_ms":10,"baseline_ms":10}]}"#,
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
            {"host":"cursor","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":10,"baseline_ms":10}
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
    let evidence = smoke_json["evidence"].as_array().unwrap();
    let mut owned_args: Vec<String> = vec!["launch-report", "--all", "--json"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for entry in evidence {
        let text = entry.as_str().unwrap();
        if let Some(path) = text.strip_prefix("ledger=") {
            owned_args.push("--ledger".into());
            owned_args.push(path.to_string());
        }
    }
    let setup_artifact = dir.path().join("setup-proof.txt");
    let invocation_artifact = dir.path().join("invocation-proof.txt");
    std::fs::write(&setup_artifact, "configured host wrapper/mcp").unwrap();
    std::fs::write(&invocation_artifact, "real host invocation smoke observed").unwrap();
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {"host":"generic_shell","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":100,"baseline_ms":100},
            {"host":"tfy_agent_adapter","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":100,"baseline_ms":100},
            {"host":"mcp_stdio","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_ms":100,"baseline_ms":100}
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
            {"host":"generic_shell","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_exception":"fixture host overhead accepted by release owner"},
            {"host":"tfy_agent_adapter","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_exception":"fixture host overhead accepted by release owner"},
            {"host":"mcp_stdio","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup-proof.txt","invocation_artifact":"invocation-proof.txt","overhead_exception":"fixture host overhead accepted by release owner"}
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
