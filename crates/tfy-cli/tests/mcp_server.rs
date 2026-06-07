use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct McpChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpChild {
    fn start(session: &str, ledger: &std::path::Path, raw_dir: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tfy"))
            .env("CARGO_TERM_COLOR", "never")
            .args([
                "mcp",
                "serve",
                "--session",
                session,
                "--ledger",
                ledger.to_str().unwrap(),
                "--raw-dir",
                raw_dir.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, value: serde_json::Value) -> serde_json::Value {
        writeln!(self.stdin, "{}", value).unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(
            line.trim_start().starts_with('{'),
            "stdout pollution: {line:?}"
        );
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for McpChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_capabilities_and_codex_dry_run_are_truthful() {
    let caps = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["mcp", "capabilities"])
        .output()
        .unwrap();
    assert!(caps.status.success());
    let caps_json: serde_json::Value = serde_json::from_slice(&caps.stdout).unwrap();
    assert_eq!(caps_json["transport"], "stdio");
    assert_eq!(caps_json["stdout_contract"], "json_rpc_only");
    assert!(caps_json["not_claimed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "private_codex_hook"));

    let dir = tempfile::tempdir().unwrap();
    let snippet = dir.path().join("codex.toml");
    let dry = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "mcp",
            "install",
            "--target",
            "codex",
            "--dry-run",
            "--output",
            snippet.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(dry.status.success());
    assert!(!snippet.exists());
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(
        text.contains("codex mcp add tfy -- tfy mcp serve"),
        "{text}"
    );
    assert!(text.contains("[mcp_servers.tfy]"), "{text}");
    assert!(text.contains("No files changed"), "{text}");
    assert!(
        text.contains("not private Codex hook interception"),
        "{text}"
    );
}

#[test]
fn mcp_initialize_tools_and_resource_templates_are_discoverable() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let mut mcp = McpChild::start("smoke", &ledger, &raw);

    let init = mcp.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "tfy");
    assert!(init["result"]["capabilities"].get("tools").is_some());
    assert!(init["result"]["capabilities"].get("resources").is_some());

    writeln!(
        mcp.stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
    )
    .unwrap();
    mcp.stdin.flush().unwrap();

    let tools = mcp.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"tfy_tool_run"));
    assert!(names.contains(&"tfy_raw_get"));
    assert!(names.contains(&"tfy_adapter_report"));

    let templates =
        mcp.request(json!({"jsonrpc":"2.0","id":3,"method":"resources/templates/list"}));
    let template_uris: Vec<_> = templates["result"]["resourceTemplates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|template| template["uriTemplate"].as_str().unwrap())
        .collect();
    assert!(template_uris.contains(&"tfy://raw/{raw_ref}"));
    assert!(template_uris.contains(&"tfy://report/{session}"));
    assert!(template_uris.contains(&"tfy://state/{session}"));

    let resources = mcp.request(json!({"jsonrpc":"2.0","id":4,"method":"resources/list"}));
    let resource_uris: Vec<_> = resources["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|resource| resource["uri"].as_str().unwrap())
        .collect();
    assert!(resource_uris.contains(&"tfy://report/smoke"));
    assert!(resource_uris.contains(&"tfy://state/smoke"));
    assert!(!resource_uris.contains(&"tfy://raw/{raw_ref}"));
}

#[test]
fn mcp_tool_run_raw_report_resources_and_failure_survival_work() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let mut mcp = McpChild::start("fail-survival", &ledger, &raw);

    let run = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":10,
        "method":"tools/call",
        "params": {"name":"tfy_tool_run", "arguments":{"command":["sh","-c","printf 'error: broken\\n'; exit 9"]}}
    }));
    assert!(run.get("result").is_some(), "{run}");
    let text = run["result"]["content"][0]["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["payload"]["exit_code"], 9);
    let raw_ref = payload["payload"]["raw_ref"].as_str().unwrap().to_string();
    assert!(text.contains("error: broken"), "{text}");

    let tools_after_failure = mcp.request(json!({"jsonrpc":"2.0","id":11,"method":"tools/list"}));
    assert!(
        tools_after_failure["result"]["tools"]
            .as_array()
            .unwrap()
            .len()
            >= 6
    );

    let resources = mcp.request(json!({"jsonrpc":"2.0","id":12,"method":"resources/list"}));
    let resource_uris: Vec<_> = resources["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|resource| resource["uri"].as_str().unwrap().to_string())
        .collect();
    assert!(resource_uris.contains(&format!("tfy://raw/{raw_ref}")));

    let raw_resource = mcp.request(json!({"jsonrpc":"2.0","id":13,"method":"resources/read","params":{"uri":format!("tfy://raw/{raw_ref}")}}));
    assert!(raw_resource["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("error: broken"));

    let report = mcp.request(json!({"jsonrpc":"2.0","id":14,"method":"resources/read","params":{"uri":"tfy://report/fail-survival"}}));
    let report_text = report["result"]["contents"][0]["text"].as_str().unwrap();
    let report_json: serde_json::Value = serde_json::from_str(report_text).unwrap();
    assert_eq!(report_json["commands"], 1);
    assert_eq!(report_json["failures"], 1);
    assert!(report_json.get("raw_bytes").is_some());
    assert!(report_json.get("model_bytes").is_some());
    assert!(report_json.get("saved_bytes").is_some());
    assert!(report_json.get("net_savings_ratio").is_some());
    assert!(report_json.get("raw_chars").is_none(), "{report_json}");

    let state = mcp.request(json!({"jsonrpc":"2.0","id":15,"method":"resources/read","params":{"uri":"tfy://state/fail-survival"}}));
    let state_text = state["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(state_text.contains("raw_ref="));
}

#[test]
fn mcp_state_projection_is_scoped_by_requested_session() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let mut mcp = McpChild::start("session-a", &ledger, &raw);

    let _ = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":20,
        "method":"tools/call",
        "params": {"name":"tfy_tool_run", "arguments":{"session":"session-a", "command":["sh","-c","printf 'alpha_marker_123'"]}}
    }));
    let _ = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":21,
        "method":"tools/call",
        "params": {"name":"tfy_tool_run", "arguments":{"session":"session-b", "command":["sh","-c","printf 'beta_marker_456'"]}}
    }));

    let state_a = mcp.request(json!({"jsonrpc":"2.0","id":22,"method":"resources/read","params":{"uri":"tfy://state/session-a"}}));
    let state_a_text = state_a["result"]["contents"][0]["text"].as_str().unwrap();
    let state_a_json: serde_json::Value = serde_json::from_str(state_a_text).unwrap();
    let state_a_evidence = state_a_json["tool_evidence"].as_array().unwrap();
    assert!(
        state_a_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("alpha_marker_123")),
        "{state_a_json}"
    );
    assert!(
        !state_a_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("printf 'beta_marker_456'")),
        "{state_a_json}"
    );

    let projected_b = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":23,
        "method":"tools/call",
        "params": {"name":"tfy_state_project", "arguments":{"session":"session-b"}}
    }));
    let text = projected_b["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let projected_b_json: serde_json::Value = serde_json::from_str(text).unwrap();
    let projected_b_evidence = projected_b_json["tool_evidence"].as_array().unwrap();
    assert!(
        projected_b_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("beta_marker_456")),
        "{projected_b_json}"
    );
    assert!(
        !projected_b_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("printf 'alpha_marker_123'")),
        "{projected_b_json}"
    );
}

#[test]
fn mcp_tool_run_uses_p0_command_family_summary_and_report() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let mut mcp = McpChild::start("mcp-family", &ledger, &raw);

    let run = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":30,
        "method":"tools/call",
        "params": {
            "name":"tfy_tool_run",
            "arguments":{
                "session":"mcp-family",
                "command":["cargo","test","--help"]
            }
        }
    }));
    assert!(run.get("result").is_some(), "{run}");
    let text = run["result"]["content"][0]["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["payload"]["command_family"], "cargo_test");
    assert_eq!(payload["payload"]["rendering_kind"], "summary");
    assert!(payload["payload"]["model_text"]
        .as_str()
        .unwrap()
        .contains("family=cargo_test"));

    let report = mcp.request(json!({"jsonrpc":"2.0","id":31,"method":"resources/read","params":{"uri":"tfy://report/mcp-family"}}));
    let report_text = report["result"]["contents"][0]["text"].as_str().unwrap();
    let report_json: serde_json::Value = serde_json::from_str(report_text).unwrap();
    assert_eq!(report_json["family_counts"]["cargo_test"], 1);
    assert_eq!(
        report_json["families_by_saved_tokens"][0]["family"],
        "cargo_test"
    );
}

fn mcp_content_json(response: &serde_json::Value) -> serde_json::Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("MCP tool response text");
    serde_json::from_str(text).unwrap()
}

#[test]
fn mcp_context_get_rejects_display_name_selectors() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let file = dir.path().join("sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):
    return price + price * tax_rate
",
    )
    .unwrap();

    let mut mcp = McpChild::start("exact-id", &ledger, &raw);
    let rejected = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":39,
        "method":"tools/call",
        "params":{"name":"tfy_context_get","arguments":{"path":file.to_str().unwrap(),"scope":"calculate_total_price"}}
    }));
    assert!(rejected.get("error").is_some(), "{rejected}");
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("exact scope id"));
}

#[test]
fn mcp_code_io_workflow_lists_context_validates_and_applies() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let file = dir.path().join("sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n\ndef keep_me(value):\n    return value\n",
    )
    .unwrap();

    let mut mcp = McpChild::start("code-io", &ledger, &raw);
    let tools = mcp.request(json!({"jsonrpc":"2.0","id":40,"method":"tools/list"}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"tfy_scope_list"));
    assert!(names.contains(&"tfy_output_apply"));

    let listed = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":41,
        "method":"tools/call",
        "params":{"name":"tfy_scope_list","arguments":{"path":file.to_str().unwrap(),"limit":1}}
    }));
    let listed_json = mcp_content_json(&listed);
    assert_eq!(listed_json["returned"], 1);
    assert_eq!(listed_json["truncated"], true);
    assert!(listed_json["selector_contract"]
        .as_str()
        .unwrap()
        .contains("scope.id"));
    let scope_id = listed_json["scopes"][0]["id"].as_str().unwrap().to_string();

    let context = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":42,
        "method":"tools/call",
        "params":{"name":"tfy_context_get","arguments":{"session":"code-io","path":file.to_str().unwrap(),"scope":scope_id,"compactness":"symbol"}}
    }));
    let compact = mcp_content_json(&context);
    assert_eq!(compact["base_compact_code"], compact["compact_code"]);
    assert_eq!(
        compact["context_ref"],
        compact["apply_proof"]["context_ref"]
    );
    assert_eq!(compact["symbol_map"]["scope_id"], compact["scope"]["id"]);
    assert!(compact["apply_proof"]["source_sha256"].as_str().is_some());

    let restore_payload = json!({
        "scope_id": compact["scope"]["id"],
        "language": compact["scope"]["language"],
        "compactness": compact["compactness"],
        "compact_code": "def f1(a,b): return a * (1 + b)",
        "base_compact_code": compact["compact_code"],
        "context_ref": compact["context_ref"],
        "symbol_map": compact["symbol_map"],
        "apply_proof": compact["apply_proof"]
    });

    let before_preview = std::fs::read_to_string(&file).unwrap();
    let preview = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":43,
        "method":"tools/call",
        "params":{"name":"tfy_output_validate","arguments":{"session":"code-io","restore_payload":restore_payload}}
    }));
    let preview_json = mcp_content_json(&preview);
    assert_eq!(preview_json["validation_status"], "non_authoritative");
    assert_eq!(preview_json["applied"], false);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before_preview);

    let applied = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":44,
        "method":"tools/call",
        "params":{"name":"tfy_output_apply","arguments":{"session":"code-io","restore_payload":restore_payload}}
    }));
    let applied_json = mcp_content_json(&applied);
    assert_eq!(applied_json["validation_status"], "valid");
    assert_eq!(applied_json["applied"], true);
    assert_eq!(
        applied_json["authority"],
        "apply_proof_and_current_file_state_only"
    );
    assert_eq!(applied_json["applied_path"], file.to_str().unwrap());
    let source = std::fs::read_to_string(&file).unwrap();
    assert!(
        source.contains("def calculate_total_price(price,tax_rate): return price * (1 + tax_rate)")
    );
    assert!(source.contains("def keep_me(value):\n    return value"));

    let state = mcp.request(json!({"jsonrpc":"2.0","id":45,"method":"resources/read","params":{"uri":"tfy://state/code-io"}}));
    let state_text = state["result"]["contents"][0]["text"].as_str().unwrap();
    let state_json: serde_json::Value = serde_json::from_str(state_text).unwrap();
    assert!(!state_json["context_refs"].as_array().unwrap().is_empty());
    let changed_files = state_json["changed_files"].as_array().unwrap();
    assert_eq!(changed_files.len(), 1, "{state_json}");
    assert!(changed_files[0].as_str().unwrap().contains("applied=true"));
    assert!(state_json["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap().contains("output preview validation")));
}

#[test]
fn mcp_output_apply_rejects_parent_event_id_without_proof() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let file = dir.path().join("sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n",
    )
    .unwrap();
    let original = std::fs::read_to_string(&file).unwrap();

    let mut mcp = McpChild::start("parent-only", &ledger, &raw);
    let listed = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":50,
        "method":"tools/call",
        "params":{"name":"tfy_scope_list","arguments":{"path":file.to_str().unwrap(),"query":"calculate_total_price"}}
    }));
    let listed_json = mcp_content_json(&listed);
    let scope_id = listed_json["scopes"][0]["id"].as_str().unwrap();
    let context = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":51,
        "method":"tools/call",
        "params":{"name":"tfy_context_get","arguments":{"path":file.to_str().unwrap(),"scope":scope_id}}
    }));
    let compact = mcp_content_json(&context);
    let no_proof_payload = json!({
        "scope_id": compact["scope"]["id"],
        "language": compact["scope"]["language"],
        "compactness": compact["compactness"],
        "compact_code": "def f1(a,b): return a",
        "base_compact_code": compact["compact_code"],
        "context_ref": compact["context_ref"],
        "symbol_map": compact["symbol_map"]
    });
    let rejected = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":52,
        "method":"tools/call",
        "params":{"name":"tfy_output_apply","arguments":{"restore_payload":no_proof_payload,"parent_event_id":"ctx-only"}}
    }));
    assert!(rejected.get("error").is_some(), "{rejected}");
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("parent_event_id"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn mcp_output_apply_rejects_stale_proof_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");
    let file = dir.path().join("sample.py");
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_rate):\n    return price + price * tax_rate\n",
    )
    .unwrap();

    let mut mcp = McpChild::start("stale", &ledger, &raw);
    let listed = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":60,
        "method":"tools/call",
        "params":{"name":"tfy_scope_list","arguments":{"path":file.to_str().unwrap(),"query":"calculate_total_price"}}
    }));
    let listed_json = mcp_content_json(&listed);
    let scope_id = listed_json["scopes"][0]["id"].as_str().unwrap();
    let context = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":61,
        "method":"tools/call",
        "params":{"name":"tfy_context_get","arguments":{"path":file.to_str().unwrap(),"scope":scope_id}}
    }));
    let compact = mcp_content_json(&context);
    std::fs::write(
        &file,
        "def calculate_total_price(price, tax_percent):\n    return price + price * tax_percent\n",
    )
    .unwrap();
    let changed = std::fs::read_to_string(&file).unwrap();
    let restore_payload = json!({
        "scope_id": compact["scope"]["id"],
        "language": compact["scope"]["language"],
        "compactness": compact["compactness"],
        "compact_code": "def f1(a,b): return a * (1 + b)",
        "base_compact_code": compact["compact_code"],
        "context_ref": compact["context_ref"],
        "symbol_map": compact["symbol_map"],
        "apply_proof": compact["apply_proof"]
    });
    let rejected = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":62,
        "method":"tools/call",
        "params":{"name":"tfy_output_apply","arguments":{"restore_payload":restore_payload}}
    }));
    assert!(rejected.get("error").is_some(), "{rejected}");
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("stale"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), changed);
}
