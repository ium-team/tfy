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
    assert!(json["ledger_events"].as_u64().unwrap() >= 3, "{json}");
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
