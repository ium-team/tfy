use std::io::Write;
use std::process::{Command, Stdio};

fn run_with_stdin(args: &[&str], stdin: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn run_plain(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn hook_shim_routes_through_same_plain_tool_gateway_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw");
    let cli = run_plain(&[
        "tool-gateway",
        "--raw-dir",
        raw.to_str().unwrap(),
        "--",
        "sh",
        "-c",
        "printf ok",
    ]);
    let adapter = run_plain(&[
        "adapter",
        "run",
        "--raw-dir",
        raw.to_str().unwrap(),
        "--session",
        "equiv",
        "--",
        "sh",
        "-c",
        "printf ok",
    ]);
    let agent = run_plain(&[
        "agent",
        "run",
        "--raw-dir",
        raw.to_str().unwrap(),
        "--session",
        "equiv",
        "--",
        "sh",
        "-c",
        "printf ok",
    ]);
    let hook = run_plain(&[
        "hook",
        "run",
        "--raw-dir",
        raw.to_str().unwrap(),
        "--session",
        "equiv",
        "--",
        "sh",
        "-c",
        "printf ok",
    ]);
    assert_eq!(cli, "ok");
    assert_eq!(adapter, cli);
    assert_eq!(agent, cli);
    assert_eq!(hook, cli);
}

#[test]
fn hook_json_records_host_hook_route_without_claiming_private_interception() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw");
    let ledger = dir.path().join("ledger.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "hook",
            "run",
            "--json",
            "--raw-dir",
            raw.to_str().unwrap(),
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "hook-json",
            "--request-id",
            "hook-r1",
            "--",
            "sh",
            "-c",
            "printf ok",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["route"]["ingress"], "host_hook");
    assert_eq!(json["origin"]["invocation"], "official_host_hook");
    assert_eq!(json["route"]["kill_switch_available"], true);
    assert_eq!(json["payload"]["model_text"], "ok");
}

#[test]
fn hook_kill_switch_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .env("TFY_HOOK_DISABLE", "1")
        .args(["hook", "run", "--", "sh", "-c", "printf ok"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TFY_HOOK_DISABLE=1"));
}

#[test]
fn hook_run_rewrites_codex_pre_tool_use_without_executing_pending_command() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw");
    let ledger = dir.path().join("ledger.jsonl");
    let side_effect = dir.path().join("side-effect");
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": format!("printf side > {}", side_effect.display())}
    });
    let output = run_with_stdin(
        &[
            "hook",
            "run",
            "--host",
            "codex",
            "--raw-dir",
            raw.to_str().unwrap(),
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "codex-hook",
        ],
        &payload.to_string(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !side_effect.exists(),
        "PreToolUse must not execute the pending command"
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rewritten = json["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    assert_eq!(json["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "allow");
    assert!(rewritten.contains(" hook run --host "), "{rewritten}");
    assert!(rewritten.contains("codex"), "{rewritten}");
    assert!(rewritten.contains(raw.to_str().unwrap()), "{rewritten}");
    assert!(rewritten.contains(ledger.to_str().unwrap()), "{rewritten}");
}

#[test]
fn rewritten_hook_command_records_official_hook_provenance_when_host_executes_it() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw");
    let ledger = dir.path().join("ledger.jsonl");
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "printf ok"}
    });
    let output = run_with_stdin(
        &[
            "hook",
            "run",
            "--host",
            "claude-code",
            "--raw-dir",
            raw.to_str().unwrap(),
            "--ledger",
            ledger.to_str().unwrap(),
            "--session",
            "claude-hook",
        ],
        &payload.to_string(),
    );
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rewritten = json["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    let executed = Command::new("sh").args(["-c", rewritten]).output().unwrap();
    assert!(
        executed.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&executed.stdout), "ok");
    let ledger_text = std::fs::read_to_string(&ledger).unwrap();
    assert!(ledger_text.contains("official_host_hook"), "{ledger_text}");
    assert!(ledger_text.contains("claude_code"), "{ledger_text}");
}

#[test]
fn hook_run_fails_closed_for_unsupported_host_even_with_explicit_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "hook",
            "run",
            "--host",
            "cursor",
            "--",
            "sh",
            "-c",
            "printf no",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not available in TFY agent mode"),
        "{stderr}"
    );
    assert!(
        stderr.contains("supported hosts: codex, claude-code"),
        "{stderr}"
    );
}

#[test]
fn hook_run_fails_closed_for_non_bash_and_ambiguous_payloads() {
    for payload in [
        r#"{"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file":"README.md"}}"#,
        r#"{"hook_event_name":"PreToolUse","tool_name":"PowerShell","tool_input":{"command":"Write-Output ok"}}"#,
        r#"{"hook_event_name":"PreToolUse","tool_name":"ShellScript","tool_input":{"command":"printf ok"}}"#,
    ] {
        let non_bash = run_with_stdin(&["hook", "run", "--host", "codex"], payload);
        assert!(!non_bash.status.success(), "payload={payload}");
        let stderr = String::from_utf8_lossy(&non_bash.stderr);
        assert!(stderr.contains("only Bash/shell"), "{stderr}");
    }

    let ambiguous = run_with_stdin(
        &["hook", "run", "--host", "claude-code"],
        r#"{"command":"printf ok"}"#,
    );
    assert!(!ambiguous.status.success());
    let stderr = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(stderr.contains("hook_event_name=PreToolUse"), "{stderr}");
}

#[test]
fn hook_run_fails_closed_on_malformed_real_host_payload() {
    let output = run_with_stdin(&["hook", "run", "--host", "claude-code"], "not-json");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("parse host hook JSON"), "{stderr}");
}

#[test]
fn hook_capabilities_and_install_are_truthful_dry_run_surfaces() {
    let caps = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["hook", "capabilities"])
        .output()
        .unwrap();
    assert!(caps.status.success());
    let json: serde_json::Value = serde_json::from_slice(&caps.stdout).unwrap();
    assert_eq!(
        json["default_policy"],
        "official_docs_backed_for_codex_and_claude_code_but_launch_evidence_gated"
    );
    assert!(json["not_claimed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "private_codex_hook"));
    let codex = json["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["target"] == "codex")
        .unwrap();
    assert_eq!(codex["claim_tier"], "config_written");

    let dry = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["hook", "install", "--target", "codex", "--dry-run"])
        .output()
        .unwrap();
    assert!(dry.status.success());
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.contains("supported_configured_unverified"));
    assert!(text.contains("kill_switch=TFY_HOOK_DISABLE=1"));

    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["hook", "install", "--target", "test-shim"])
        .output()
        .unwrap();
    assert!(!apply.status.success());
    assert!(String::from_utf8_lossy(&apply.stderr).contains("dry-run only"));
}
