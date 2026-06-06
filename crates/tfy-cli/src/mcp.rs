use crate::adapter::build_adapter_report;
use crate::gateways::tool_gateway_envelopes;
use crate::util::{print_json, stable_id};
use anyhow::{bail, Result};
use clap::Subcommand;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use tfy_core::*;
use tfy_runtime::*;

#[derive(Subcommand)]
pub(crate) enum McpCmd {
    /// Print supported MCP server capabilities without starting stdio serving.
    Capabilities,
    /// Run TFY as an MCP stdio server. Stdout is JSON-RPC only; diagnostics go to stderr.
    Serve {
        #[arg(long, default_value = "local-session")]
        session: String,
        #[arg(long, default_value = ".tfy/mcp/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long = "max-summary-bytes", default_value_t = 1_000_000)]
        max_summary_bytes: usize,
    },
    /// Print a reversible Codex MCP setup snippet. Dry-run by default unless --output is used.
    Install {
        #[arg(long, default_value = "codex")]
        target: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "local-session")]
        session: String,
    },
}

pub(crate) fn execute_mcp(cmd: McpCmd) -> Result<()> {
    match cmd {
        McpCmd::Capabilities => print_json(&mcp_capabilities()),
        McpCmd::Serve {
            session,
            ledger,
            raw_dir,
            max_summary_bytes,
        } => serve_mcp(session, ledger, raw_dir, max_summary_bytes),
        McpCmd::Install {
            target,
            dry_run,
            output,
            session,
        } => execute_mcp_install(&target, dry_run, output, &session),
    }
}

fn mcp_capabilities() -> serde_json::Value {
    serde_json::json!({
        "adapter_version": env!("CARGO_PKG_VERSION"),
        "transport": "stdio",
        "stdout_contract": "json_rpc_only",
        "logs": "stderr_or_file_only",
        "automatic_interception": "mcp_host_routing_required_not_private_hook",
        "supported_targets": ["mcp-stdio", "codex-setup-snippet"],
        "not_claimed": ["private_codex_hook", "provider_prompt_gateway", "universal_shell_interception"],
        "tools": mcp_tools(),
        "resource_templates": mcp_resource_templates(),
    })
}

pub(crate) fn execute_mcp_install(
    target: &str,
    dry_run: bool,
    output: Option<PathBuf>,
    session: &str,
) -> Result<()> {
    if target != "codex" {
        bail!("mcp install target '{target}' is not implemented; supported target: codex");
    }
    let command_line = format!(
        "codex mcp add tfy -- tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw"
    );
    let toml = format!(
        "[mcp_servers.tfy]\ncommand = \"tfy\"\nargs = [\"mcp\", \"serve\", \"--session\", \"{session}\", \"--ledger\", \".tfy/mcp/ledger.jsonl\", \"--raw-dir\", \".tfy/raw\"]\n"
    );
    let text = format!(
        "TFY MCP Codex setup dry-run\n\n{command_line}\n\n# Equivalent ~/.codex/config.toml snippet\n{toml}\nNo files changed by this dry-run. Apply manually with the command above or copy the TOML snippet. Remove/revert by deleting the tfy MCP server entry from Codex config. This is MCP tool/resource integration, not private Codex hook interception, provider prompt interception, or universal shell command interception.\n"
    );
    if let Some(path) = output {
        if dry_run {
            print!("{text}");
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&path, toml)?;
        println!("wrote Codex MCP snippet to {}", path.display());
        println!("manual apply command: {command_line}");
        println!("remove/revert by deleting the tfy MCP server entry from Codex config");
        println!("boundary: MCP integration only; no private Codex hook interception claimed");
    } else {
        print!("{text}");
    }
    Ok(())
}

fn serve_mcp(
    session: String,
    ledger: PathBuf,
    raw_dir: PathBuf,
    max_summary_bytes: usize,
) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(request) => {
                let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
                if method.starts_with("notifications/") && request.get("id").is_none() {
                    continue;
                }
                handle_mcp_request(&request, &session, &ledger, &raw_dir, max_summary_bytes)
            }
            Err(err) => mcp_error(
                serde_json::Value::Null,
                -32700,
                format!("parse error: {err}"),
            ),
        };
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle_mcp_request(
    request: &serde_json::Value,
    session: &str,
    ledger: &PathBuf,
    raw_dir: &PathBuf,
    max_summary_bytes: usize,
) -> serde_json::Value {
    let id = request
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = request
        .get("params")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let result = match method {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("2025-06-18"),
            "serverInfo": {"name": "tfy", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"subscribe": false, "listChanged": false}
            }
        })),
        "tools/list" => Ok(serde_json::json!({"tools": mcp_tools()})),
        "resources/list" => mcp_resources_list(session, ledger),
        "resources/templates/list" => {
            Ok(serde_json::json!({"resourceTemplates": mcp_resource_templates()}))
        }
        "tools/call" => mcp_tools_call(params, session, ledger, raw_dir, max_summary_bytes),
        "resources/read" => mcp_resources_read(params, session, ledger, raw_dir),
        "notifications/initialized" => Ok(serde_json::json!({})),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    match result {
        Ok(result) => serde_json::json!({"jsonrpc":"2.0", "id": id, "result": result}),
        Err((code, message)) => mcp_error(id, code, message),
    }
}

fn mcp_error(id: serde_json::Value, code: i32, message: String) -> serde_json::Value {
    serde_json::json!({"jsonrpc":"2.0", "id": id, "error": {"code": code, "message": message}})
}

fn mcp_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"name":"tfy_tool_run","description":"Run an ordinary command through TFY Tool Gateway; raw output is stored locally and model-facing output is compact only when smaller.","inputSchema":{"type":"object","properties":{"command":{"type":"array","items":{"type":"string"}},"session":{"type":"string"}},"required":["command"]}}),
        serde_json::json!({"name":"tfy_raw_get","description":"Recover raw output by raw_ref.","inputSchema":{"type":"object","properties":{"raw_ref":{"type":"string"},"around":{"type":"string"},"context":{"type":"integer"}},"required":["raw_ref"]}}),
        serde_json::json!({"name":"tfy_context_get","description":"Get compact or full context for a file scope.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"scope":{"type":"string"},"compactness":{"type":"string"}},"required":["path","scope"]}}),
        serde_json::json!({"name":"tfy_output_validate","description":"Validate/restore compact code output without applying changes.","inputSchema":{"type":"object","properties":{"restore_payload":{"type":"object"}},"required":["restore_payload"]}}),
        serde_json::json!({"name":"tfy_state_project","description":"Project compact task state from the TFY ledger.","inputSchema":{"type":"object","properties":{"session":{"type":"string"}}}}),
        serde_json::json!({"name":"tfy_adapter_report","description":"Report measured byte savings for an MCP session.","inputSchema":{"type":"object","properties":{"session":{"type":"string"}}}}),
    ]
}

fn mcp_resource_templates() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"uriTemplate":"tfy://raw/{raw_ref}","name":"TFY raw command output","description":"Recover locally stored raw command output by raw_ref","mimeType":"text/plain"}),
        serde_json::json!({"uriTemplate":"tfy://report/{session}","name":"TFY session savings report","description":"Byte-level TFY savings report for a session","mimeType":"application/json"}),
        serde_json::json!({"uriTemplate":"tfy://state/{session}","name":"TFY state projection","description":"Compact task state projection for a session","mimeType":"application/json"}),
    ]
}

fn mcp_resources_list(
    session: &str,
    ledger: &PathBuf,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let mut resources = vec![
        serde_json::json!({"uri": format!("tfy://report/{session}"), "name": format!("TFY report {session}"), "mimeType":"application/json"}),
        serde_json::json!({"uri": format!("tfy://state/{session}"), "name": format!("TFY state {session}"), "mimeType":"application/json"}),
    ];
    if let Ok(events) = load_events(ledger) {
        for event in events.iter().filter(|event| event.session_id == session) {
            for raw_ref in &event.provenance.raw_refs {
                resources.push(serde_json::json!({"uri": format!("tfy://raw/{raw_ref}"), "name": format!("TFY raw {raw_ref}"), "mimeType":"text/plain"}));
            }
        }
    }
    Ok(serde_json::json!({"resources": resources}))
}

fn mcp_tools_call(
    params: serde_json::Value,
    default_session: &str,
    ledger: &PathBuf,
    raw_dir: &PathBuf,
    max_summary_bytes: usize,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match name {
        "tfy_tool_run" => {
            let command: Vec<String> = serde_json::from_value(
                args.get("command")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| (-32602, format!("invalid command: {e}")))?;
            let session = args
                .get("session")
                .and_then(|v| v.as_str())
                .unwrap_or(default_session)
                .to_string();
            let (event, response, _exit_code) = tool_gateway_envelopes(
                command,
                raw_dir.clone(),
                max_summary_bytes,
                session,
                None,
                None,
                None,
                AdapterKind::Mcp,
            )
            .map_err(|e| (-32000, e.to_string()))?;
            if let Err(err) = append_event(ledger, &event) {
                eprintln!("tfy mcp warning: could not append ledger event: {err}");
            }
            Ok(mcp_tool_content(
                serde_json::to_value(response).map_err(|e| (-32000, e.to_string()))?,
            ))
        }
        "tfy_raw_get" => {
            let raw_ref = args
                .get("raw_ref")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (-32602, "raw_ref is required".to_string()))?;
            let around = args.get("around").and_then(|v| v.as_str());
            let context = args.get("context").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
            let bytes = raw_output_bytes(raw_dir, raw_ref, around, context)
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(mcp_tool_content(bytes_to_safe_json(raw_ref, bytes)))
        }
        "tfy_context_get" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (-32602, "path is required".to_string()))?;
            let scope = args
                .get("scope")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (-32602, "scope is required".to_string()))?;
            let compactness = args
                .get("compactness")
                .and_then(|v| v.as_str())
                .unwrap_or("symbol");
            let exp = expand_scope(PathBuf::from(path), scope, compactness)
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(mcp_tool_content(
                serde_json::to_value(exp).map_err(|e| (-32000, e.to_string()))?,
            ))
        }
        "tfy_output_validate" => {
            let restore_payload_value = args
                .get("restore_payload")
                .cloned()
                .ok_or_else(|| (-32602, "restore_payload is required".to_string()))?;
            let payload: RestorePayload = serde_json::from_value(restore_payload_value)
                .map_err(|e| (-32602, e.to_string()))?;
            let restored = restore_payload(payload).map_err(|e| (-32000, e.to_string()))?;
            let patch_ref = stable_id(&restored.restored_code);
            Ok(mcp_tool_content(serde_json::json!({
                "validation_status":"non_authoritative",
                "applied":false,
                "restored_code":restored.restored_code,
                "patch_ref": patch_ref,
                "warnings":["MCP output validation is preview-only; workspace apply and full benefit validation require explicit authority/provenance gates"]
            })))
        }
        "tfy_state_project" => {
            let session = args
                .get("session")
                .and_then(|v| v.as_str())
                .unwrap_or(default_session);
            let events = load_events(ledger).map_err(|e| (-32000, e.to_string()))?;
            let scoped = scoped_events(&events, session);
            Ok(mcp_tool_content(
                serde_json::to_value(project_state(&scoped))
                    .map_err(|e| (-32000, e.to_string()))?,
            ))
        }
        "tfy_adapter_report" => {
            let session = args
                .get("session")
                .and_then(|v| v.as_str())
                .unwrap_or(default_session);
            let report = build_adapter_report(ledger.clone(), session)
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(mcp_tool_content(
                serde_json::to_value(report).map_err(|e| (-32000, e.to_string()))?,
            ))
        }
        _ => Err((-32602, format!("unknown tool: {name}"))),
    }
}

fn mcp_tool_content(value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())}]})
}

fn scoped_events(
    events: &[RuntimeEnvelope<GatewayEvent>],
    session: &str,
) -> Vec<RuntimeEnvelope<GatewayEvent>> {
    events
        .iter()
        .filter(|event| event.session_id == session)
        .cloned()
        .collect()
}

fn mcp_resources_read(
    params: serde_json::Value,
    default_session: &str,
    ledger: &PathBuf,
    raw_dir: &PathBuf,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "uri is required".to_string()))?;
    let contents = if let Some(raw_ref) = uri.strip_prefix("tfy://raw/") {
        let bytes =
            raw_output_bytes(raw_dir, raw_ref, None, 3).map_err(|e| (-32000, e.to_string()))?;
        resource_text_or_blob(uri, bytes)
    } else if let Some(session) = uri.strip_prefix("tfy://report/") {
        let session = if session.is_empty() {
            default_session
        } else {
            session
        };
        let report =
            build_adapter_report(ledger.clone(), session).map_err(|e| (-32000, e.to_string()))?;
        vec![
            serde_json::json!({"uri": uri, "mimeType":"application/json", "text": serde_json::to_string_pretty(&report).map_err(|e| (-32000, e.to_string()))?}),
        ]
    } else if let Some(session) = uri.strip_prefix("tfy://state/") {
        let session = if session.is_empty() {
            default_session
        } else {
            session
        };
        let events = load_events(ledger).map_err(|e| (-32000, e.to_string()))?;
        let scoped = scoped_events(&events, session);
        let projection = project_state(&scoped);
        vec![
            serde_json::json!({"uri": uri, "mimeType":"application/json", "text": serde_json::to_string_pretty(&projection).map_err(|e| (-32000, e.to_string()))?}),
        ]
    } else {
        return Err((-32602, format!("unsupported resource uri: {uri}")));
    };
    Ok(serde_json::json!({"contents": contents}))
}

fn bytes_to_safe_json(raw_ref: &str, bytes: Vec<u8>) -> serde_json::Value {
    match String::from_utf8(bytes.clone()) {
        Ok(text) => serde_json::json!({"raw_ref": raw_ref, "text": text, "bytes": bytes.len()}),
        Err(_) => {
            serde_json::json!({"raw_ref": raw_ref, "encoding":"base64", "data": base64_encode(&bytes), "bytes": bytes.len()})
        }
    }
}

fn resource_text_or_blob(uri: &str, bytes: Vec<u8>) -> Vec<serde_json::Value> {
    match String::from_utf8(bytes.clone()) {
        Ok(text) => vec![serde_json::json!({"uri": uri, "mimeType":"text/plain", "text": text})],
        Err(_) => vec![
            serde_json::json!({"uri": uri, "mimeType":"application/octet-stream", "blob": base64_encode(&bytes)}),
        ],
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
