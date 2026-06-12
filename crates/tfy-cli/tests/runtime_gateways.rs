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
    assert_eq!(lines[0]["route"]["ingress"], "cli_gateway");
    assert_eq!(lines[0]["route"]["claim_tier"], "route_evidence_recorded");
    assert_eq!(lines[0]["route"]["no_negative_savings_proven"], true);
    assert_eq!(lines[1]["payload"]["kind"], "tool_command");
    assert_eq!(lines[1]["route"]["ingress"], "cli_gateway");
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
    assert_eq!(shell_json["route"]["ingress"], "generic_shell_adapter");
    assert_eq!(shell_json["route"]["kill_switch_available"], true);

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
    assert_eq!(
        output_json["provenance"]["apply_authority"]["parent_event_id_only"],
        true
    );
    assert_eq!(
        output_json["provenance"]["apply_authority"]["validate_succeeded"],
        false
    );
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
    assert_eq!(
        json["provenance"]["apply_authority"]["parent_event_id_only"],
        false
    );
    assert_eq!(
        json["provenance"]["apply_authority"]["validate_succeeded"],
        false
    );
}

fn python_apply_fixture() -> (tempfile::TempDir, std::path::PathBuf, serde_json::Value) {
    let fixture = tempfile::tempdir().unwrap();
    let file = fixture.path().join("apply_sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n\ndef keep_me(value):\n    return value\n",
    )
    .unwrap();
    let context = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "context-gateway",
            file.to_str().unwrap(),
            "calculate_total_price",
            "--request-id",
            "ctx-apply",
            "--trace-id",
            "trace-apply",
        ])
        .output()
        .unwrap();
    assert!(
        context.status.success(),
        "{}",
        String::from_utf8_lossy(&context.stderr)
    );
    let context_json: serde_json::Value = serde_json::from_slice(&context.stdout).unwrap();
    (fixture, file, context_json)
}

#[test]
fn output_gateway_apply_replaces_only_proven_selected_scope() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let compact = context_json["payload"]["compact_context"].clone();
    assert!(compact["apply_proof"]["source_sha256"].as_str().is_some());
    assert!(compact["apply_proof"]["context_ref"]
        .as_str()
        .unwrap()
        .starts_with("tfy_"));

    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a * (1 + b)",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            payload.path().to_str().unwrap(),
            "--request-id",
            "out-apply",
            "--trace-id",
            "trace-apply",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["validation_status"], "valid");
    assert_eq!(json["payload"]["applied"], true);
    assert_eq!(json["provenance"]["validation_status"], "valid");
    assert_eq!(json["payload"]["applied_path"], file.to_str().unwrap());
    assert!(json["payload"]["before_hash"].as_str().is_some());
    assert!(json["payload"]["after_hash"].as_str().is_some());
    assert_eq!(
        json["provenance"]["apply_authority"]["context_ref"],
        compact["apply_proof"]["context_ref"]
    );
    assert_eq!(
        json["provenance"]["apply_authority"]["base_content_hash"],
        compact["apply_proof"]["source_sha256"]
    );
    assert_eq!(
        json["provenance"]["apply_authority"]["validate_succeeded"],
        true
    );
    assert_eq!(
        json["provenance"]["apply_authority"]["parent_event_id_only"],
        false
    );

    let source = std::fs::read_to_string(file).unwrap();
    assert!(
        source.contains("def calculate_total_price(price,tax_rate): return price * (1 + tax_rate)")
    );
    assert!(source.contains("def keep_me(value):\n    return value"));
}

#[test]
fn output_gateway_apply_requires_canonical_content_proof_and_writes_nothing() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let original = std::fs::read_to_string(&file).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();

    let mut no_proof = tempfile::NamedTempFile::new().unwrap();
    write!(
        no_proof,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"]
        })
    )
    .unwrap();
    let parent_only = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            no_proof.path().to_str().unwrap(),
            "--parent-event-id",
            "ctx-apply",
        ])
        .output()
        .unwrap();
    assert!(!parent_only.status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);

    let proof_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        proof_file.path(),
        serde_json::to_string(&compact["apply_proof"]).unwrap(),
    )
    .unwrap();
    let mut double_proof = tempfile::NamedTempFile::new().unwrap();
    write!(
        double_proof,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let ambiguous = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            double_proof.path().to_str().unwrap(),
            "--context-proof",
            proof_file.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!ambiguous.status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn output_gateway_apply_allows_unrelated_file_changes_when_selected_hash_matches() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let original = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{}\n# unrelated trailer\n", original)).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a - b",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
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
    let source = std::fs::read_to_string(file).unwrap();
    assert!(source.contains("return price - tax_rate"));
    assert!(source.contains("# unrelated trailer"));
}

#[test]
fn output_gateway_apply_rejects_stale_and_unmapped_without_writing() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let original = std::fs::read_to_string(&file).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();
    std::fs::write(&file, original.replace("tax_rate", "tax_percent")).unwrap();

    let mut stale_payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        stale_payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let stale = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            stale_payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let stale_source = std::fs::read_to_string(&file).unwrap();
    assert!(stale_source.contains("tax_percent"));
    assert!(!stale_source.contains("price * (1 + tax_rate)"));

    std::fs::write(&file, &original).unwrap();
    let mut unmapped_payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        unmapped_payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return v99",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let unmapped = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            unmapped_payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!unmapped.status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn output_gateway_apply_rejects_forged_base_context_without_writing() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let original = std::fs::read_to_string(&file).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a - b",
            "base_compact_code": "def f1(a,b): return forged",
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(file).unwrap(), original);
}

#[test]
fn output_gateway_apply_rejects_mutated_proof_compact_hash_without_writing() {
    let (_fixture, file, context_json) = python_apply_fixture();
    let original = std::fs::read_to_string(&file).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();
    let forged_base = "def f1(a,b): return forged";
    let forged_hash = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(forged_base.as_bytes()))
    };
    let mut forged_proof = compact["apply_proof"].clone();
    forged_proof["compact_code_sha256"] = serde_json::Value::String(forged_hash);
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a - b",
            "base_compact_code": forged_base,
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": forged_proof
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(file).unwrap(), original);
}

#[test]
fn output_gateway_apply_rejects_symbol_map_from_wrong_scope() {
    let fixture = tempfile::tempdir().unwrap();
    let file_one = fixture.path().join("one.py");
    let file_two = fixture.path().join("two.py");
    std::fs::write(
        &file_one,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n",
    )
    .unwrap();
    std::fs::write(
        &file_two,
        "def unrelated(alpha, beta):\n    return alpha - beta\n",
    )
    .unwrap();
    let one = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "context-gateway",
            file_one.to_str().unwrap(),
            "calculate_total_price",
        ])
        .output()
        .unwrap();
    assert!(
        one.status.success(),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    let two = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["context-gateway", file_two.to_str().unwrap(), "unrelated"])
        .output()
        .unwrap();
    assert!(
        two.status.success(),
        "{}",
        String::from_utf8_lossy(&two.stderr)
    );
    let one_json: serde_json::Value = serde_json::from_slice(&one.stdout).unwrap();
    let two_json: serde_json::Value = serde_json::from_slice(&two.stdout).unwrap();
    let one_compact = one_json["payload"]["compact_context"].clone();
    let two_compact = two_json["payload"]["compact_context"].clone();
    let original = std::fs::read_to_string(&file_one).unwrap();

    let mut forged_symbol_map = two_compact["symbol_map"].clone();
    forged_symbol_map["scope_id"] = one_compact["scope"]["id"].clone();
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": one_compact["scope"]["id"],
            "language": one_compact["scope"]["language"],
            "compactness": one_compact["compactness"],
            "compact_code": "def f1(a,b): return a - b",
            "base_compact_code": one_compact["compact_code"],
            "context_ref": one_compact["apply_proof"]["context_ref"],
            "symbol_map": forged_symbol_map,
            "apply_proof": one_compact["apply_proof"]
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
            "--payload",
            payload.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&file_one).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn output_gateway_apply_preserves_target_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let (_fixture, file, context_json) = python_apply_fixture();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    let compact = context_json["payload"]["compact_context"].clone();
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id": compact["scope"]["id"],
            "language": compact["scope"]["language"],
            "compactness": compact["compactness"],
            "compact_code": "def f1(a,b): return a * (1 + b)",
            "base_compact_code": compact["compact_code"],
            "context_ref": compact["apply_proof"]["context_ref"],
            "symbol_map": compact["symbol_map"],
            "apply_proof": compact["apply_proof"]
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "output-gateway",
            "--apply",
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
    assert_eq!(
        std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
