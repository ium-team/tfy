use std::io::Write;
use std::process::{Command, Stdio};

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
    assert!(text.contains("official_host_hook_guidance"), "{text}");

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
fn smoke_codex_is_checklist_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["smoke", "--codex"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("checklist/report-only"), "{text}");
    assert!(
        text.contains("until real Codex hook invocation evidence exists"),
        "{text}"
    );
}

#[test]
fn setup_named_hosts_emit_truthful_snippets_without_claiming_savings() {
    let cases = [
        ("codex", "[[hooks.PreToolUse]]"),
        ("claude-code", "\"PreToolUse\""),
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
fn setup_cursor_remains_planned_discovery() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["setup", "--ai", "--host", "cursor", "--dry-run"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "host 'cursor' is not available in TFY agent mode; supported hosts: codex, claude-code"
        ),
        "{stderr}"
    );
}

#[test]
fn cursor_doctor_and_smoke_are_unsupported_not_configurable() {
    let doctor = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["doctor", "--host", "cursor", "--json"])
        .output()
        .unwrap();
    assert!(!doctor.status.success());
    assert!(
        String::from_utf8_lossy(&doctor.stderr).contains(
            "host 'cursor' is not available in TFY agent mode; supported hosts: codex, claude-code"
        ),
        "stderr={}",
        String::from_utf8_lossy(&doctor.stderr)
    );

    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["smoke", "--host", "cursor", "--json"])
        .output()
        .unwrap();
    assert!(!smoke.status.success());
    assert!(
        String::from_utf8_lossy(&smoke.stderr).contains(
            "host 'cursor' is not available in TFY agent mode; supported hosts: codex, claude-code"
        ),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
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
    for hidden in ["cursor", "opencode", "hermes", "openclaw"] {
        assert!(
            hosts.iter().all(|host| host["host"] != hidden),
            "unexpected public host {hidden}: {json}"
        );
    }
    assert_eq!(find("codex")["claim_tier"], "configurable");
    assert_eq!(find("claude-code")["claim_tier"], "configurable");
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
        .any(|ingress| ingress == "official_host_hook"));
    assert!(find("claude-code")["supported_ingress"]
        .as_array()
        .unwrap()
        .iter()
        .any(|ingress| ingress == "official_host_hook"));
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
        r#"{"hosts":[{"host":"claude-code","tfy_version":"0.1.1","setup_verified":true,"real_invocation_verified":true,"setup_artifact":"setup.txt","invocation_artifact":"invoke.txt","overhead_ms":10,"baseline_ms":10}]}"#,
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
    let claude_code = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "claude-code")
        .unwrap();
    assert_ne!(claude_code["status"], "launch_supported", "{json}");
    assert!(
        json["blockers"].as_array().unwrap().iter().any(|b| b
            .as_str()
            .unwrap()
            .contains("no command-output savings data")),
        "{json}"
    );
}

#[test]
fn launch_report_keeps_cursor_planned_discovery_even_with_host_evidence() {
    let dir = tempfile::tempdir().unwrap();
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
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert!(
        json["host_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .all(|host| host["host"] != "cursor"),
        "{json}"
    );
    assert!(
        json["host_evidence"]["named_hosts"]["cursor"].is_null(),
        "{json}"
    );
    assert!(
        !json.to_string().contains("cursor"),
        "unsupported host leaked into public launch report: {json}"
    );
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
fn gain_reads_default_agent_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "agent",
            "run",
            "--session",
            "agent-default-gain",
            "--",
            "sh",
            "-c",
            "yes line | head -n 80",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());

    let gain = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["gain", "--session", "agent-default-gain", "--json"])
        .output()
        .unwrap();
    assert!(gain.status.success());
    let json: serde_json::Value = serde_json::from_slice(&gain.stdout).unwrap();
    assert_eq!(json["commands"], 1);
    assert!(
        json["ledgers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|ledger| ledger == ".tfy/agent/ledger.jsonl"),
        "{json}"
    );
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
    assert_eq!(adapter["status"], "verified_local_route");
    assert!(adapter["launch_claim"]
        .as_str()
        .unwrap()
        .contains("local smoke verified"));
    assert_eq!(json["status"], "blocked");
    assert!(json["blockers"].as_array().unwrap().iter().any(|b| {
        b.as_str()
            .unwrap()
            .contains("required v1 host tfy_agent_adapter")
    }));
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
            .any(|h| h["host"] == "tfy_agent_adapter" && h["status"] == "verified_local_route"),
        "{json}"
    );
    assert_eq!(json["host_evidence"]["tfy_agent_adapter"], true);
    assert_eq!(json["host_evidence"]["generic_shell_wrapper"], false);
}

#[test]
fn launch_report_refuses_named_host_unsupported_route_type_even_with_artifacts_and_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "claude-code-config.json",
        "claude-code-ledger.jsonl",
        "claude-code-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"claude-code",
              "tfy_version":"0.1.1",
              "host_id":"claude-code",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"claude-code-config.json",
              "route_type":"provider_api_prompt_proxy",
              "ledger_artifact":"claude-code-ledger.jsonl",
              "raw_artifact":"claude-code-raw.txt",
              "smoke_id":"claude-code-provider-proxy-smoke",
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
    let claude_code = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "claude-code")
        .unwrap();
    assert_ne!(claude_code["status"], "launch_supported", "{json}");
    assert!(!claude_code["evidence_tiers"]
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
fn launch_report_promotes_codex_and_claude_code_official_hook_evidence() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "codex-setup.txt",
        "codex-invocation.txt",
        "codex-config.toml",
        "codex-ledger.jsonl",
        "codex-raw.txt",
        "claude-setup.txt",
        "claude-invocation.txt",
        "claude-settings.json",
        "claude-ledger.jsonl",
        "claude-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"codex",
              "tfy_version":"0.1.1",
              "host_id":"codex",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"codex-setup.txt",
              "invocation_artifact":"codex-invocation.txt",
              "config_scope":"project",
              "config_path":"codex-config.toml",
              "route_type":"official_host_hook",
              "ledger_artifact":"codex-ledger.jsonl",
              "raw_artifact":"codex-raw.txt",
              "smoke_id":"codex-hook-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "official_docs_backed":true,
              "kill_switch_available":true,
              "uninstall_available":true,
              "overhead_ms":10,
              "baseline_ms":10
            },
            {
              "host":"claude-code",
              "tfy_version":"0.1.1",
              "host_id":"claude-code",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"claude-setup.txt",
              "invocation_artifact":"claude-invocation.txt",
              "config_scope":"project",
              "config_path":"claude-settings.json",
              "route_type":"official_host_hook",
              "ledger_artifact":"claude-ledger.jsonl",
              "raw_artifact":"claude-raw.txt",
              "smoke_id":"claude-hook-smoke",
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
    for host_name in ["codex", "claude-code"] {
        let host = json["host_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .find(|host| host["host"] == host_name)
            .unwrap();
        assert_eq!(host["status"], "launch_supported", "{json}");
        let tiers = host["evidence_tiers"].as_array().unwrap();
        assert!(
            tiers.iter().any(|tier| tier == "verified_host_hook"),
            "{json}"
        );
        assert!(
            tiers.iter().any(|tier| tier == "launch_supported"),
            "{json}"
        );
        assert!(host["evidence_gate"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gate| gate
                .as_str()
                .unwrap()
                .contains("route_type=official_host_hook")));
        assert_eq!(
            json["host_evidence"]["named_hosts"][host_name]["host_bound_evidence"], true,
            "{json}"
        );
    }
    assert!(json["host_evidence"]["evidence_notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|note| note.contains("codex") && note.contains("hook_authorized=true")));
}

#[test]
fn launch_report_refuses_official_hook_promotion_without_supported_hook_authority() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "setup-proof.txt",
        "invocation-proof.txt",
        "claude-code-config.json",
        "claude-code-ledger.jsonl",
        "claude-code-raw.txt",
    ] {
        std::fs::write(dir.path().join(file), "proof").unwrap();
    }
    let host_evidence = dir.path().join("host-evidence.json");
    std::fs::write(
        &host_evidence,
        r#"{
          "hosts": [
            {
              "host":"claude-code",
              "tfy_version":"0.1.1",
              "host_id":"claude-code",
              "host_version":"test",
              "setup_verified":true,
              "real_invocation_verified":true,
              "setup_artifact":"setup-proof.txt",
              "invocation_artifact":"invocation-proof.txt",
              "config_scope":"project",
              "config_path":"claude-code-config.json",
              "route_type":"official_host_hook",
              "ledger_artifact":"claude-code-ledger.jsonl",
              "raw_artifact":"claude-code-raw.txt",
              "smoke_id":"claude-code-hook-smoke",
              "timestamp":"2026-06-09T00:00:00Z",
              "redacted_public_bytes":1000,
              "model_visible_bytes":100,
              "official_docs_backed":false,
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
    let claude_code = json["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == "claude-code")
        .unwrap();
    assert_ne!(claude_code["status"], "launch_supported", "{json}");
    assert!(!claude_code["evidence_tiers"]
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
fn bench_manifest_is_self_benchmark_and_fails_closed_for_public_comparison_claim() {
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
    assert_eq!(json["public_superiority_claim_ready"], false);
    assert!(json["claim_policy"]
        .as_str()
        .unwrap()
        .contains("fail closed"));
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
        json["release_tiers"]["beta_ready"]["status"], "ready",
        "{json}"
    );
    assert_eq!(
        json["release_tiers"]["developer_preview_ready"], json["release_tiers"]["beta_ready"],
        "legacy developer_preview_ready field should alias beta_ready"
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
        .any(|b| b.as_str().unwrap().contains("no named AI host")));
    assert_eq!(
        json["release_tiers"]["public_superiority_claim_ready"]["status"],
        "blocked"
    );
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
        start_text.contains("default_agent_routes_configured_verification_required"),
        "{start_text}"
    );
    assert!(
        start_text.contains("Installed TFY agent command wrapper"),
        "{start_text}"
    );
    assert!(
        start_text.contains("Configured Codex project PreToolUse Bash hook route"),
        "{start_text}"
    );
    assert!(
        start_text.contains("Configured Claude Code project PreToolUse Bash hook route"),
        "{start_text}"
    );
    assert!(
        start_text.contains("ordinary_terminal_interception=false"),
        "{start_text}"
    );
    assert!(
        start_text.contains("managed_session_interception=false"),
        "{start_text}"
    );
    assert!(start_text.contains("globally intercepted"), "{start_text}");
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
        "default_agent_routes_configured_verification_required"
    );
    assert!(dir.path().join(".tfy/agent/tfy-agent-wrapper").exists());
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
    let human_support_status = if cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )) {
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
        .args(["start", "agent", "--host", "codex", "--apply"])
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
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )) {
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
        .args(["start", "agent", "--host", "all", "--verify"])
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
fn lifecycle_start_agent_host_claude_code_applies_project_hook_with_verification_required() {
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
    assert!(dir.path().join(".claude/settings.json").exists());
    assert!(!dir.path().join(".codex/config.toml").exists());
    assert!(!dir.path().join(".tfy/agent/tfy-agent-wrapper").exists());
    let script = dir.path().join(".tfy/agent/claude-pre-tool-use");
    assert!(script.exists());
    let script_text = std::fs::read_to_string(&script).unwrap();
    assert!(script_text.contains("--host claude-code"), "{script_text}");
    assert!(
        script_text.contains(dir.path().join(".tfy/raw").to_str().unwrap()),
        "{script_text}"
    );
    assert!(
        script_text.contains(
            dir.path()
                .join(".tfy/hook/claude-code-ledger.jsonl")
                .to_str()
                .unwrap()
        ),
        "{script_text}"
    );
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
    assert_eq!(claude["route_type"], "official_host_hook", "{json}");
    assert_eq!(claude["configured"], true, "{json}");
    assert_eq!(claude["route_configured"], true, "{json}");
    assert_eq!(claude["host_approval_required"], false, "{json}");
    assert_eq!(claude["active"], false, "{json}");
}

#[test]
fn lifecycle_start_agent_claude_preserves_similar_unowned_hook_without_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let claude_dir = dir.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "/user/managed/.tfy/agent/claude-pre-tool-use",
        "timeout": 30
      }]
    }]
  }
}
"#,
    )
    .unwrap();
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
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude_dir.join("settings.json")).unwrap())
            .unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{settings}");
    assert!(settings
        .to_string()
        .contains("/user/managed/.tfy/agent/claude-pre-tool-use"));
}

#[test]
fn lifecycle_start_agent_unsupported_host_is_guidance_only_without_configured_claim() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "--agent", "--host", "cursor"])
        .output()
        .unwrap();
    assert!(!start.status.success());
    assert!(String::from_utf8_lossy(&start.stderr).contains("not available in TFY agent mode"));
    let status = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["status", "--agent", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    if let Some(agent) = json["project_lifecycle"]["agent"].as_object() {
        assert!(
            agent
                .get("host_routes")
                .and_then(serde_json::Value::as_object)
                .is_none_or(serde_json::Map::is_empty),
            "{json}"
        );
    }
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
    if cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )) {
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
    let managed_session_available = cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ));
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
        "current_directory_scoped_tfy_managed_session"
    );
    let expected_shell = if cfg!(target_os = "macos") {
        "macos-zsh"
    } else if cfg!(target_os = "windows") {
        "windows-powershell"
    } else {
        "linux-bash"
    };
    assert_eq!(human["shells_supported"][0], expected_shell);
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
    assert!(!dir.path().join(".tfy/human/auto-activate.json").exists());
    assert!(!dir.path().join(".tfy/human/auto-activate.bash").exists());
    assert!(!dir.path().join(".tfy/human/auto-activate.zsh").exists());
    assert!(!dir.path().join(".tfy/human/auto-activate.ps1").exists());
}

#[test]
fn lifecycle_status_exposes_derived_activation_vocabulary() {
    let dir = tempfile::tempdir().unwrap();
    let start = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["start", "agent", "--host", "codex", "--apply"])
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
fn live_host_smoke_generates_codex_launch_report_evidence_with_fake_host() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake_codex = bin_dir.join("codex");
    std::fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
if [ "${1:-}" = "--version" ]; then
  echo "codex-cli fake-live-smoke"
  exit 0
fi
payload='{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"for i in $(seq 1 80); do echo tfy-live-host-hook-smoke-$i; done"}}'
out=$(printf '%s' "$payload" | ./.tfy/agent/codex-pre-tool-use)
cmd=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["updatedInput"]["command"])')
sh -c "$cmd"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("PATH", path)
        .args(["smoke", "--host", "codex", "--live", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    assert_eq!(json["mode"], "host_live", "{json}");
    let host = &json["hosts"][0];
    assert_eq!(host["status"], "pass", "{json}");
    assert_eq!(host["host"], "codex", "{json}");
    let evidence = host["host_evidence"].as_str().unwrap();
    assert!(std::path::Path::new(evidence).is_file(), "{json}");

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["launch-report", "--json", "--host-evidence", evidence])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let launch: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let codex = launch["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "codex")
        .unwrap();
    assert_eq!(codex["status"], "launch_supported", "{launch}");
    assert_eq!(
        launch["host_evidence"]["named_hosts"]["codex"]["host_bound_evidence"], true,
        "{launch}"
    );
}

#[cfg(unix)]
#[test]
fn live_host_smoke_rejects_extra_fake_host_hook_command() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake_codex = bin_dir.join("codex");
    std::fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
if [ "${1:-}" = "--version" ]; then
  echo "codex-cli fake-live-smoke"
  exit 0
fi
payload='{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"for i in $(seq 1 80); do echo tfy-live-host-hook-smoke-$i; done"}}'
out=$(printf '%s' "$payload" | ./.tfy/agent/codex-pre-tool-use)
cmd=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["updatedInput"]["command"])')
sh -c "$cmd"
extra='{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"for i in $(seq 1 80); do echo tfy-live-host-hook-extra-$i; done"}}'
out=$(printf '%s' "$extra" | ./.tfy/agent/codex-pre-tool-use)
cmd=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["updatedInput"]["command"])')
sh -c "$cmd"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("PATH", path)
        .args(["smoke", "--host", "codex", "--live", "--json"])
        .output()
        .unwrap();
    assert!(
        !smoke.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&smoke.stdout)
    );
    let stderr = String::from_utf8_lossy(&smoke.stderr);
    assert!(
        stderr.contains("expected exactly one official-host-hook command"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn live_host_smoke_rejects_wrong_fake_host_hook_command() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake_codex = bin_dir.join("codex");
    std::fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
if [ "${1:-}" = "--version" ]; then
  echo "codex-cli fake-live-smoke"
  exit 0
fi
payload='{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"for i in $(seq 1 80); do echo tfy-live-host-hook-wrong-$i; done"}}'
out=$(printf '%s' "$payload" | ./.tfy/agent/codex-pre-tool-use)
cmd=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["updatedInput"]["command"])')
sh -c "$cmd"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("PATH", path)
        .args(["smoke", "--host", "codex", "--live", "--json"])
        .output()
        .unwrap();
    assert!(
        !smoke.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&smoke.stdout)
    );
    let stderr = String::from_utf8_lossy(&smoke.stderr);
    assert!(
        stderr.contains("official hook command mismatch"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn live_host_smoke_generates_claude_code_launch_report_evidence_with_fake_host() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake_claude = bin_dir.join("claude");
    std::fs::write(
        &fake_claude,
        r#"#!/usr/bin/env sh
set -eu
if [ "${1:-}" = "--version" ]; then
  echo "2.1.160 fake-live-smoke"
  exit 0
fi
payload='{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"for i in $(seq 1 80); do echo tfy-live-host-hook-smoke-$i; done"}}'
out=$(printf '%s' "$payload" | ./.tfy/agent/claude-pre-tool-use)
cmd=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["updatedInput"]["command"])')
sh -c "$cmd"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake_claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("PATH", path)
        .args(["smoke", "--host", "claude-code", "--live", "--json"])
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&smoke.stdout).unwrap();
    assert_eq!(json["status"], "pass", "{json}");
    assert_eq!(json["mode"], "host_live", "{json}");
    let host = &json["hosts"][0];
    assert_eq!(host["status"], "pass", "{json}");
    assert_eq!(host["host"], "claude-code", "{json}");
    let evidence = host["host_evidence"].as_str().unwrap();
    assert!(std::path::Path::new(evidence).is_file(), "{json}");

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["launch-report", "--json", "--host-evidence", evidence])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let launch: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let claude = launch["host_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "claude-code")
        .unwrap();
    assert_eq!(claude["status"], "launch_supported", "{launch}");
    assert_eq!(
        launch["host_evidence"]["named_hosts"]["claude-code"]["host_bound_evidence"], true,
        "{launch}"
    );
}
