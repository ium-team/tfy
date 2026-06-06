use std::io::Write;
use std::process::Command;

#[test]
fn runtime_capabilities_and_negotiation_are_structured() {
    let caps = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .arg("runtime-capabilities")
        .output()
        .unwrap();
    assert!(caps.status.success());
    let caps_json: serde_json::Value = serde_json::from_slice(&caps.stdout).unwrap();
    assert!(caps_json["supported_gateways"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "tool"));

    let negotiation = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "runtime-negotiate",
            "--gateway",
            "tool",
            "--output-mode",
            "json",
        ])
        .output()
        .unwrap();
    assert!(negotiation.status.success());
    let json: serde_json::Value = serde_json::from_slice(&negotiation.stdout).unwrap();
    assert_eq!(json["accepted"], true);
}

#[test]
fn tool_gateway_jsonl_emits_event_response_and_state_projection() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "tool-gateway",
            "--jsonl",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--session-id",
            "s1",
            "--request-id",
            "r1",
            "--trace-id",
            "t1",
            "--",
            "sh",
            "-c",
            "printf ok",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["payload"]["kind"], "tool_command_completed");
    assert_eq!(lines[1]["payload"]["kind"], "tool_command");
    assert_eq!(lines[1]["request_id"], "r1");
    assert!(lines[1]["provenance"]["raw_refs"].as_array().unwrap().len() == 1);

    let shell = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "shell",
            "--json",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "printf shell-ok",
        ])
        .output()
        .unwrap();
    assert!(shell.status.success());
    let shell_json: serde_json::Value = serde_json::from_slice(&shell.stdout).unwrap();
    assert_eq!(shell_json["payload"]["kind"], "tool_command");
    assert_eq!(shell_json["payload"]["summary"], "shell-ok");
    assert_eq!(shell_json["payload"]["model_text"], "shell-ok");
    assert_eq!(shell_json["payload"]["rendering_kind"], "pass_through");

    let projection = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["state-project", "--ledger", ledger.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(projection.status.success());
    let json: serde_json::Value = serde_json::from_slice(&projection.stdout).unwrap();
    assert_eq!(json["payload"]["kind"], "state_projection");
    assert!(json["payload"]["projection"]["tool_evidence"][0]
        .as_str()
        .unwrap()
        .contains("raw_ref="));
    assert_eq!(json["payload"]["projection"]["authoritative"], true);
    assert_eq!(
        json["payload"]["projection"]["provenance"]["validation_status"],
        "valid"
    );
}

#[test]
fn context_and_output_gateways_return_runtime_envelopes() {
    let fixture = tempfile::tempdir().unwrap();
    let file = fixture.path().join("sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n",
    )
    .unwrap();
    let context = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "context-gateway",
            file.to_str().unwrap(),
            "calculate_total_price",
            "--request-id",
            "ctx1",
            "--trace-id",
            "trace1",
        ])
        .output()
        .unwrap();
    assert!(
        context.status.success(),
        "{}",
        String::from_utf8_lossy(&context.stderr)
    );
    let context_json: serde_json::Value = serde_json::from_slice(&context.stdout).unwrap();
    assert_eq!(context_json["payload"]["kind"], "context");
    assert_eq!(context_json["request_id"], "ctx1");
    assert!(context_json["payload"]["context_ref"]
        .as_str()
        .unwrap()
        .starts_with("tfy_"));

    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id":"sample.py:1:2:function:calculate_total_price",
            "language":"python",
            "compactness":"symbol",
            "compact_code":"def f0(a,b): return a+a*b",
            "symbols":{"calculate_total_price":"f0","price":"a","tax_rate":"b"}
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--payload",
            payload.path().to_str().unwrap(),
            "--request-id",
            "out1",
            "--trace-id",
            "trace1",
            "--parent-event-id",
            "ctx1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output_json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output_json["payload"]["kind"], "output");
    assert_eq!(output_json["parent_event_id"], "ctx1");
    assert_eq!(output_json["payload"]["validation_status"], "valid");
    assert!(output_json["payload"]["restored_code"]
        .as_str()
        .unwrap()
        .contains("calculate_total_price"));
}

#[test]
fn context_ref_changes_when_scope_content_changes() {
    let fixture = tempfile::tempdir().unwrap();
    let file = fixture.path().join("sample.py");
    std::fs::write(&file, "def f(x):\n    return x + 1\n").unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["context-gateway", file.to_str().unwrap(), "f"])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let first_ref = first_json["payload"]["context_ref"]
        .as_str()
        .unwrap()
        .to_string();

    std::fs::write(&file, "def f(x):\n    return x + 2\n").unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["context-gateway", file.to_str().unwrap(), "f"])
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    let second_ref = second_json["payload"]["context_ref"].as_str().unwrap();
    assert_ne!(first_ref, second_ref);
}

#[test]
fn output_gateway_without_parent_provenance_is_non_authoritative() {
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id":"s",
            "language":"python",
            "compactness":"symbol",
            "compact_code":"def f0(a): return a",
            "symbols":{"identity":"f0","value":"a"}
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--payload",
            payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["validation_status"], "non_authoritative");
    assert_eq!(json["provenance"]["validation_status"], "non_authoritative");
}
