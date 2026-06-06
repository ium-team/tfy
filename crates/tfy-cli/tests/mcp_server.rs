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
        "params": {"name":"tfy_tool_run", "arguments":{"session":"session-a", "command":["sh","-c","printf 'aaa'"]}}
    }));
    let _ = mcp.request(json!({
        "jsonrpc":"2.0",
        "id":21,
        "method":"tools/call",
        "params": {"name":"tfy_tool_run", "arguments":{"session":"session-b", "command":["sh","-c","printf 'bbb'"]}}
    }));

    let state_a = mcp.request(json!({"jsonrpc":"2.0","id":22,"method":"resources/read","params":{"uri":"tfy://state/session-a"}}));
    let state_a_text = state_a["result"]["contents"][0]["text"].as_str().unwrap();
    let state_a_json: serde_json::Value = serde_json::from_str(state_a_text).unwrap();
    let state_a_evidence = state_a_json["tool_evidence"].as_array().unwrap();
    assert!(
        state_a_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("aaa")),
        "{state_a_json}"
    );
    assert!(
        !state_a_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("bbb")),
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
            .any(|line| line.as_str().unwrap().contains("bbb")),
        "{projected_b_json}"
    );
    assert!(
        !projected_b_evidence
            .iter()
            .any(|line| line.as_str().unwrap().contains("aaa")),
        "{projected_b_json}"
    );
}
