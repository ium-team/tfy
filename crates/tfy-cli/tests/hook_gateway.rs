use std::process::Command;

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
fn hook_run_fails_closed_for_real_hosts_until_official_e2e_support_exists() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "hook",
            "run",
            "--host",
            "codex",
            "--",
            "sh",
            "-c",
            "printf ok",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("supported only for test-shim"), "{stderr}");
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
        "disabled_until_official_host_docs_and_e2e_evidence"
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
    assert_eq!(codex["claim_tier"], "unsupported");

    let dry = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["hook", "install", "--target", "codex", "--dry-run"])
        .output()
        .unwrap();
    assert!(dry.status.success());
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.contains("unsupported_without_public_official_hook"));
    assert!(text.contains("kill_switch=TFY_HOOK_DISABLE=1"));

    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["hook", "install", "--target", "test-shim"])
        .output()
        .unwrap();
    assert!(!apply.status.success());
    assert!(String::from_utf8_lossy(&apply.stderr).contains("dry-run only"));
}
