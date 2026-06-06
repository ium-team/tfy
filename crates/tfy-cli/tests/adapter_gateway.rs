use std::process::Command;

#[test]
fn adapter_run_intercepts_command_without_json_leakage_and_records_report() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "run",
            "--session",
            "s-adapter",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
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
    assert!(!model_text.contains("protocol_version"), "{model_text}");
    assert!(!model_text.trim_start().starts_with('{'), "{model_text}");

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "s-adapter",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["session"], "s-adapter");
    assert_eq!(json["commands"], 1);
    assert_eq!(json["rendering_counts"]["pass_through"], 1);
    assert!(json["raw_refs"].as_array().unwrap().len() == 1);
}

#[test]
fn adapter_run_summarizes_long_output_and_report_shows_positive_savings() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "run",
            "--session",
            "long-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
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

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "long-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["rendering_counts"]["summary"], 1);
    assert!(
        json["estimated_saved_tokens"].as_i64().unwrap() > 0,
        "{json}"
    );
    assert!(json["savings_pct"].as_f64().unwrap() > 0.0, "{json}");
}

#[test]
fn adapter_run_preserves_failure_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "adapter",
            "run",
            "--session",
            "fail-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf 'error: broken\n'; exit 9",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("error: broken"), "{model_text}");

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "fail-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["failures"], 1);
}

#[test]
fn adapter_install_generic_shell_dry_run_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("tfy-agent-shell");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "install",
            "--target",
            "generic-shell",
            "--dry-run",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output_path.exists());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("dry-run"), "{text}");
    assert!(text.contains("target=generic-shell"), "{text}");
    assert!(text.contains("tfy adapter run"), "{text}");
}

#[test]
fn adapter_capabilities_marks_generic_shell_and_mcp_supported() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["adapter", "capabilities"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["default_model_visible_output"], "plain_text");
    let targets = json["targets"].as_array().unwrap();
    assert!(targets
        .iter()
        .any(|target| target["target"] == "generic-shell" && target["status"] == "supported"));
    assert!(targets.iter().any(|target| target["target"] == "mcp"
        && target["status"] == "supported"
        && target["automatic_interception"] == "mcp_host_routing_required_not_private_hook"));
    assert!(targets
        .iter()
        .any(|target| target["target"] == "provider" && target["status"] != "supported"));
}

#[test]
fn adapter_run_ledger_failure_preserves_command_output_and_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "run",
            "--session",
            "bad-ledger",
            "--ledger",
            dir.path().to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf ok; exit 7",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not append ledger event"));
}

#[test]
fn adapter_report_uses_byte_consistent_metrics_for_multibyte_output() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "run",
            "--session",
            "utf8-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf '한글'",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, "한글".as_bytes());

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "utf8-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(report.status.success());
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["raw_bytes"], 6);
    assert_eq!(json["model_bytes"], 6);
    assert_eq!(json["saved_bytes"], 0);
    assert_eq!(json["net_savings_ratio"], 0.0);
    assert!(json.get("raw_chars").is_none(), "{json}");
    assert!(json.get("model_chars").is_none(), "{json}");
    assert_eq!(json["savings_pct"], 0.0);
}

#[test]
fn adapter_report_loads_legacy_tool_command_completed_events() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    std::fs::write(
        &ledger,
        serde_json::json!({
            "protocol_version": "0.1.0",
            "adapter_kind": "cli",
            "adapter_version": "0.1.0",
            "supported_gateways": ["tool", "state"],
            "authority_mode": "execute_with_runtime_authority",
            "session_id": "legacy",
            "turn_id": null,
            "request_id": "legacy-r1",
            "parent_event_id": null,
            "trace_id": "legacy-t1",
            "workspace_root": null,
            "provenance": {"raw_refs": ["cmdout_deadbeef0000_0000000000000000"], "validation_status": "valid"},
            "policy": {
                "max_tokens": null,
                "max_output_bytes": 1000000,
                "redact_public": true,
                "require_fallback_refs": true,
                "destructive_action_requires_confirmation": true,
                "adapter_enabled": true
            },
            "payload": {
                "kind": "tool_command_completed",
                "command": "printf ok",
                "exit_code": 0,
                "risk": "success",
                "raw_ref": "cmdout_deadbeef0000_0000000000000000",
                "summary_chars": 2
            }
        })
        .to_string()
            + "
",
    )
    .unwrap();
    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "legacy",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["commands"], 1);
    assert_eq!(json["model_bytes"], 2);
    assert_eq!(json["saved_bytes"], 0);
    assert_eq!(json["estimated_saved_tokens"], 0);
    assert_eq!(json["net_savings_ratio"], 0.0);
    assert_eq!(json["rendering_counts"]["legacy_unknown"], 1);
    assert_eq!(json["family_counts"]["generic"], 1);
    assert_eq!(json["families_by_saved_tokens"][0]["family"], "generic");
    assert_eq!(json["families_by_saved_tokens"][0]["saved_bytes"], 0);
}

#[test]
fn adapter_report_includes_deterministic_family_savings() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "adapter",
            "run",
            "--session",
            "family-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
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
    let model_text = String::from_utf8_lossy(&output.stdout);
    assert!(model_text.contains("family=cargo_test"), "{model_text}");

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "adapter",
            "report",
            "--session",
            "family-session",
            "--ledger",
            ledger.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["family_counts"]["cargo_test"], 1);
    let families = json["families_by_saved_tokens"].as_array().unwrap();
    assert_eq!(families[0]["family"], "cargo_test");
    assert!(families[0]["saved_bytes"].as_i64().unwrap() > 0, "{json}");
    assert!(
        families[0]["estimated_saved_tokens"].as_i64().unwrap() > 0,
        "{json}"
    );
}
