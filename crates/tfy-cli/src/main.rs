use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::PathBuf;
use tfy_core::*;
use tfy_runtime::*;

#[derive(Parser)]
#[command(name = "tfy", about = "Token-efficient AI work interface")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum AdapterCmd {
    /// Print supported adapter targets and their current implementation status.
    Capabilities,
    /// Print or create a reversible local command shim for an agent runtime.
    Install {
        #[arg(long, default_value = "generic-shell")]
        target: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Run an ordinary command through TFY as an agent command-boundary adapter.
    Run {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long, default_value = ".tfy/adapter/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "local-session")]
        session: String,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long)]
        parent_event_id: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        jsonl: bool,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Report measured session savings from adapter ledger events.
    Report {
        #[arg(long, default_value = ".tfy/adapter/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "local-session")]
        session: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum McpCmd {
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

#[derive(Subcommand)]
enum Cmd {
    /// Agent-runtime adapter commands for opt-in automatic command-boundary interception.
    Adapter {
        #[command(subcommand)]
        cmd: AdapterCmd,
    },
    /// MCP stdio server and Codex setup commands for agent-native tool/resource integration.
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
    Index {
        path: PathBuf,
    },
    Expand {
        path: PathBuf,
        scope: String,
        #[arg(long, default_value = "symbol")]
        compactness: String,
    },
    Full {
        path: PathBuf,
        scope: String,
    },
    Restore {
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    DecideContext {
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    Run {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Tool Gateway entrypoint for agent runtimes that proxy ordinary commands through TFY.
    ToolGateway {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        jsonl: bool,
        #[arg(long, default_value = ".tfy/state/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "local-session")]
        session_id: String,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long)]
        parent_event_id: Option<String>,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Shell adapter wrapper for runtimes that configure command execution through TFY.
    Shell {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        jsonl: bool,
        #[arg(long, default_value = ".tfy/state/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "local-session")]
        session_id: String,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long)]
        parent_event_id: Option<String>,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Print runtime adapter capabilities used for gateway negotiation.
    RuntimeCapabilities,
    /// Negotiate required gateway support with the built-in CLI adapter.
    RuntimeNegotiate {
        #[arg(long, default_value = "tool")]
        gateway: String,
        #[arg(long, default_value = "json")]
        output_mode: String,
        #[arg(long, default_value_t = true)]
        adapter_enabled: bool,
        #[arg(long, default_value = tfy_runtime::RUNTIME_PROTOCOL_VERSION)]
        protocol_version: String,
    },
    /// Runtime-facing Context Gateway over index/expand/full/decide-context primitives.
    ContextGateway {
        path: PathBuf,
        scope: String,
        #[arg(long, default_value = "symbol")]
        compactness: String,
        #[arg(long, default_value = "")]
        diagnostics: String,
        #[arg(long, default_value = "local-session")]
        session_id: String,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
    },
    /// Runtime-facing Output Gateway: restore/validate compact structured code without applying.
    OutputGateway {
        #[arg(long)]
        payload: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value = "local-session")]
        session_id: String,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long)]
        parent_event_id: Option<String>,
    },
    /// Append a runtime GatewayEvent envelope to the State Gateway ledger.
    StateAppend {
        #[arg(long, default_value = ".tfy/state/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    /// Project a compact task state from the State Gateway event ledger.
    StateProject {
        #[arg(long, default_value = ".tfy/state/ledger.jsonl")]
        ledger: PathBuf,
    },
    Raw {
        raw_ref: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long)]
        around: Option<String>,
        #[arg(long, default_value_t = 3)]
        context: usize,
    },
    Languages,
    EvalCode {
        path: PathBuf,
        scope: String,
        #[arg(long, default_value = "symbol")]
        compactness: String,
    },
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Adapter { cmd } => execute_adapter(cmd)?,
        Cmd::Mcp { cmd } => execute_mcp(cmd)?,
        Cmd::Index { path } => print_json(&index_path(path)?)?,
        Cmd::Expand {
            path,
            scope,
            compactness,
        } => print_json(&expand_scope(path, &scope, &compactness)?)?,
        Cmd::Full { path, scope } => print_json(&full_scope(path, &scope)?)?,
        Cmd::Restore { payload } => {
            let text = read_payload(payload)?;
            let p: RestorePayload = serde_json::from_str(&text)?;
            print_json(&restore_payload(p)?)?;
        }
        Cmd::DecideContext { payload } => {
            let text = read_payload(payload)?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            print_json(&context_decision_from_value(&v)?)?;
        }
        Cmd::Run {
            raw_dir,
            max_summary_bytes,
            command,
        } => execute_plain_tool_gateway(command, raw_dir, max_summary_bytes)?,
        Cmd::ToolGateway {
            raw_dir,
            max_summary_bytes,
            json,
            jsonl,
            ledger,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
            command,
        }
        | Cmd::Shell {
            raw_dir,
            max_summary_bytes,
            json,
            jsonl,
            ledger,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
            command,
        } => execute_structured_tool_gateway(
            command,
            raw_dir,
            max_summary_bytes,
            json,
            jsonl,
            ledger,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
        )?,
        Cmd::RuntimeCapabilities => print_json(&AdapterCapabilities::cli_default())?,
        Cmd::RuntimeNegotiate {
            gateway,
            output_mode,
            adapter_enabled,
            protocol_version,
        } => {
            let req = NegotiationRequest {
                required_protocol_version: protocol_version,
                required_gateways: vec![parse_gateway(&gateway)?],
                required_output_mode: parse_output_mode(&output_mode)?,
                minimum_authority_mode: AuthorityMode::ObserveOnly,
                require_redaction: true,
                require_raw_store: true,
                adapter_enabled,
            };
            print_json(&negotiate(&AdapterCapabilities::cli_default(), &req))?;
        }
        Cmd::ContextGateway {
            path,
            scope,
            compactness,
            diagnostics,
            session_id,
            request_id,
            trace_id,
        } => execute_context_gateway(
            path,
            scope,
            compactness,
            diagnostics,
            session_id,
            request_id,
            trace_id,
        )?,
        Cmd::OutputGateway {
            payload,
            apply,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
        } => execute_output_gateway(
            payload,
            apply,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
        )?,
        Cmd::StateAppend { ledger, payload } => {
            let text = read_payload(payload)?;
            let event: RuntimeEnvelope<GatewayEvent> = serde_json::from_str(&text)?;
            validate_envelope(&event)?;
            append_event(ledger, &event)?;
        }
        Cmd::StateProject { ledger } => {
            let events = load_events(ledger)?;
            let projection = project_state(&events);
            let mut provenance = projection.provenance.clone();
            provenance.validation_status = Some(if projection.authoritative {
                ValidationStatus::Valid
            } else {
                ValidationStatus::NonAuthoritative
            });
            let response = response_envelope(
                GatewayResponse::StateProjection {
                    projection: Box::new(projection),
                },
                "local-session",
                &stable_id("state-project"),
                &stable_id("state-trace"),
                None,
                provenance,
            );
            print_json(&response)?;
        }
        Cmd::Raw {
            raw_ref,
            raw_dir,
            around,
            context,
        } => {
            let bytes = raw_output_bytes(raw_dir, &raw_ref, around.as_deref(), context)?;
            io::stdout().write_all(&bytes)?;
        }
        Cmd::Languages => print_json(&serde_json::json!({"languages": supported_languages()}))?,
        Cmd::EvalCode {
            path,
            scope,
            compactness,
        } => {
            let exp = expand_scope(&path, &scope, &compactness)?;
            let full = full_scope(&path, &scope)?;
            print_json(
                &serde_json::json!({"scope":exp.scope,"evaluation":evaluate_code(&full.code,&exp.compact_code),"char_metrics":exp.metrics}),
            )?;
        }
    }
    Ok(())
}

fn execute_adapter(cmd: AdapterCmd) -> Result<()> {
    match cmd {
        AdapterCmd::Capabilities => print_json(&serde_json::json!({
            "adapter_version": env!("CARGO_PKG_VERSION"),
            "default_model_visible_output": "plain_text",
            "json_policy": "debug_adapter_internal_only",
            "canonical_execution": ["tfy tool-gateway", "tfy shell"],
            "targets": [
                {
                    "target": "generic-shell",
                    "status": "supported",
                    "automatic_interception": "configured_command_wrapper",
                    "gateways": ["tool", "state"],
                    "claim_gate": "adapter e2e tests"
                },
                {
                    "target": "codex",
                    "status": "planned",
                    "automatic_interception": "not_claimed_without_host_e2e_tests"
                },
                {
                    "target": "mcp",
                    "status": "supported",
                    "automatic_interception": "mcp_host_routing_required_not_private_hook",
                    "gateways": ["tool", "context", "output", "state"],
                    "transport": "stdio",
                    "claim_gate": "mcp_server_fixture_tests"
                },
                {
                    "target": "editor",
                    "status": "planned",
                    "automatic_interception": "not_claimed_without_host_e2e_tests"
                },
                {
                    "target": "provider",
                    "status": "planned_optional_adapter",
                    "automatic_interception": "not_core_correctness"
                }
            ]
        })),
        AdapterCmd::Install {
            target,
            dry_run,
            output,
        } => execute_adapter_install(&target, dry_run, output),
        AdapterCmd::Run {
            raw_dir,
            max_summary_bytes,
            ledger,
            session,
            request_id,
            trace_id,
            parent_event_id,
            json,
            jsonl,
            command,
        } => execute_structured_tool_gateway(
            command,
            raw_dir,
            max_summary_bytes,
            json,
            jsonl,
            ledger,
            session,
            request_id,
            trace_id,
            parent_event_id,
        ),
        AdapterCmd::Report {
            ledger,
            session,
            json,
        } => execute_adapter_report(ledger, &session, json),
    }
}

fn execute_adapter_install(target: &str, dry_run: bool, output: Option<PathBuf>) -> Result<()> {
    if target != "generic-shell" {
        bail!("adapter target '{target}' is not implemented; supported target: generic-shell");
    }
    let script = r#"#!/usr/bin/env sh
# TFY generic-shell adapter shim. Agent runtimes can configure this file
# as their command wrapper; all arguments after this script are the original command.
exec tfy adapter run --session "${TFY_SESSION_ID:-local-session}" -- "$@"
"#;
    if dry_run || output.is_none() {
        let path = output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "./tfy-agent-shell".into());
        println!("TFY adapter install dry-run");
        println!("target=generic-shell status=supported");
        println!("would_write={path}");
        println!(
            "configure your agent runtime command wrapper to call: {path} -- <ordinary command>"
        );
        println!(
            "script:
{script}"
        );
        return Ok(());
    }
    let Some(output) = output else {
        bail!("adapter install requires --output unless --dry-run is used");
    };
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    if output.exists() {
        let backup = output.with_extension("bak");
        fs::copy(&output, &backup)?;
    }
    fs::write(&output, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&output, fs::Permissions::from_mode(0o755))?;
    }
    println!(
        "installed generic-shell adapter shim at {}",
        output.display()
    );
    Ok(())
}

#[derive(Default, serde::Serialize)]
struct AdapterReport {
    session: String,
    commands: usize,
    failures: usize,
    raw_bytes: usize,
    model_bytes: usize,
    saved_bytes: isize,
    net_savings_ratio: f64,
    estimated_raw_tokens: usize,
    estimated_model_tokens: usize,
    estimated_saved_tokens: isize,
    savings_pct: f64,
    negative_savings_avoided: usize,
    rendering_counts: BTreeMap<String, usize>,
    raw_refs: Vec<String>,
}

fn build_adapter_report(ledger: PathBuf, session: &str) -> Result<AdapterReport> {
    let events = load_events(ledger)?;
    let mut report = AdapterReport {
        session: session.into(),
        ..Default::default()
    };
    for event in events.iter().filter(|event| event.session_id == session) {
        if let GatewayEvent::ToolCommandCompleted {
            exit_code,
            raw_ref,
            raw_bytes,
            model_bytes,
            raw_chars,
            summary_chars,
            model_chars,
            rendering_kind,
            negative_savings_avoided,
            ..
        } = &event.payload
        {
            report.commands += 1;
            if *exit_code != 0 {
                report.failures += 1;
            }
            let raw_size = if *raw_bytes == 0 {
                *raw_chars
            } else {
                *raw_bytes
            };
            let model_size = if *model_bytes != 0 {
                *model_bytes
            } else if *model_chars != 0 {
                *model_chars
            } else {
                *summary_chars
            };
            report.raw_bytes += raw_size;
            report.model_bytes += model_size;
            if *negative_savings_avoided {
                report.negative_savings_avoided += 1;
            }
            let rendering_kind = if rendering_kind.is_empty() {
                "legacy_unknown".to_string()
            } else {
                rendering_kind.clone()
            };
            *report.rendering_counts.entry(rendering_kind).or_insert(0) += 1;
            report.raw_refs.push(raw_ref.clone());
        }
    }
    report.saved_bytes = report.raw_bytes as isize - report.model_bytes as isize;
    report.net_savings_ratio = if report.raw_bytes == 0 {
        0.0
    } else {
        report.saved_bytes as f64 / report.raw_bytes as f64
    };
    report.estimated_raw_tokens = estimate_tokens(report.raw_bytes);
    report.estimated_model_tokens = estimate_tokens(report.model_bytes);
    report.estimated_saved_tokens =
        report.estimated_raw_tokens as isize - report.estimated_model_tokens as isize;
    report.savings_pct = report.net_savings_ratio * 100.0;
    Ok(report)
}

fn execute_adapter_report(ledger: PathBuf, session: &str, json: bool) -> Result<()> {
    let report = build_adapter_report(ledger, session)?;
    if json {
        print_json(&report)
    } else {
        println!("TFY adapter report session={}", report.session);
        println!("commands={} failures={}", report.commands, report.failures);
        println!(
            "raw_bytes={} model_bytes={} savings_pct={:.2}",
            report.raw_bytes, report.model_bytes, report.savings_pct
        );
        println!(
            "estimated_raw_tokens={} estimated_model_tokens={} estimated_saved_tokens={}",
            report.estimated_raw_tokens,
            report.estimated_model_tokens,
            report.estimated_saved_tokens
        );
        println!(
            "negative_savings_avoided={} rendering_counts={:?}",
            report.negative_savings_avoided, report.rendering_counts
        );
        if !report.raw_refs.is_empty() {
            println!("raw_refs={}", report.raw_refs.join(","));
        }
        Ok(())
    }
}

fn execute_mcp(cmd: McpCmd) -> Result<()> {
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

fn execute_mcp_install(
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

fn estimate_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

fn execute_plain_tool_gateway(
    command: Vec<String>,
    raw_dir: PathBuf,
    max_summary_bytes: usize,
) -> Result<()> {
    let summary = run_tool_command(command, raw_dir, max_summary_bytes)?;
    print!("{}", summary.summary);
    io::stdout().flush()?;
    std::process::exit(summary.exit_code);
}

#[allow(clippy::too_many_arguments)]
fn execute_structured_tool_gateway(
    command: Vec<String>,
    raw_dir: PathBuf,
    max_summary_bytes: usize,
    json: bool,
    jsonl: bool,
    ledger: PathBuf,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
    parent_event_id: Option<String>,
) -> Result<()> {
    let (event, response, exit_code) = tool_gateway_envelopes(
        command,
        raw_dir,
        max_summary_bytes,
        session_id,
        request_id,
        trace_id,
        parent_event_id,
        AdapterKind::Cli,
    )?;
    if let Err(err) = append_event(ledger, &event) {
        eprintln!("tfy adapter warning: could not append ledger event: {err}");
    }
    if jsonl {
        println!("{}", serde_json::to_string(&event)?);
        println!("{}", serde_json::to_string(&response)?);
    } else if json {
        print_json(&response)?;
    } else {
        if let GatewayResponse::ToolCommand { model_text, .. } = &response.payload {
            print!("{}", model_text);
        }
    }
    io::stdout().flush()?;
    std::process::exit(exit_code);
}

#[allow(clippy::too_many_arguments)]
fn tool_gateway_envelopes(
    command: Vec<String>,
    raw_dir: PathBuf,
    max_summary_bytes: usize,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
    parent_event_id: Option<String>,
    adapter_kind: AdapterKind,
) -> Result<(
    RuntimeEnvelope<GatewayEvent>,
    RuntimeEnvelope<GatewayResponse>,
    i32,
)> {
    let summary = run_tool_command(command, raw_dir, max_summary_bytes)?;
    let request_id = request_id.unwrap_or_else(|| {
        stable_id(&format!(
            "tool:{}:{}:{}",
            session_id, summary.command, summary.raw_ref
        ))
    });
    let trace_id = trace_id.unwrap_or_else(|| request_id.clone());
    let mut provenance = ProvenanceRefs {
        raw_refs: vec![summary.raw_ref.clone()],
        validation_status: Some(ValidationStatus::Valid),
        ..Default::default()
    };
    let response = gateway_response_envelope(
        GatewayResponse::ToolCommand {
            command: summary.command.clone(),
            exit_code: summary.exit_code,
            risk: summary.risk.clone(),
            summary: summary.summary.clone(),
            model_text: summary.model_text.clone(),
            rendering_kind: summary.rendering_kind.clone(),
            raw_ref: summary.raw_ref.clone(),
            evidence: summary.evidence.clone(),
        },
        &session_id,
        &request_id,
        &trace_id,
        parent_event_id.clone(),
        provenance.clone(),
        adapter_kind.clone(),
    );
    provenance.source_event_ids = vec![request_id.clone()];
    let event = gateway_event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: summary.command,
            exit_code: summary.exit_code,
            risk: summary.risk,
            raw_ref: summary.raw_ref,
            raw_bytes: summary.raw_chars,
            model_bytes: summary.model_text.len(),
            raw_chars: summary.raw_chars,
            summary_chars: summary.summary_chars,
            model_chars: summary.model_text.len(),
            savings_pct: summary.savings_pct,
            negative_savings_avoided: summary.rendering_kind == "pass_through"
                && summary.raw_chars <= summary.summary_chars,
            rendering_kind: summary.rendering_kind,
        },
        &session_id,
        &request_id,
        &trace_id,
        parent_event_id,
        provenance,
        adapter_kind,
    );
    let exit_code = match &response.payload {
        GatewayResponse::ToolCommand { exit_code, .. } => *exit_code,
        _ => 1,
    };
    Ok((event, response, exit_code))
}

fn gateway_event_envelope(
    payload: GatewayEvent,
    session_id: &str,
    request_id: &str,
    trace_id: &str,
    parent_event_id: Option<String>,
    provenance: ProvenanceRefs,
    adapter_kind: AdapterKind,
) -> RuntimeEnvelope<GatewayEvent> {
    let mut envelope = RuntimeEnvelope::new(
        adapter_kind,
        vec![
            GatewayKind::Tool,
            GatewayKind::Context,
            GatewayKind::Output,
            GatewayKind::State,
        ],
        AuthorityMode::ExecuteWithRuntimeAuthority,
        session_id,
        request_id,
        trace_id,
        payload,
    );
    envelope.parent_event_id = parent_event_id;
    envelope.provenance = provenance;
    envelope
}

fn gateway_response_envelope(
    payload: GatewayResponse,
    session_id: &str,
    request_id: &str,
    trace_id: &str,
    parent_event_id: Option<String>,
    provenance: ProvenanceRefs,
    adapter_kind: AdapterKind,
) -> RuntimeEnvelope<GatewayResponse> {
    let mut envelope = RuntimeEnvelope::new(
        adapter_kind,
        vec![
            GatewayKind::Tool,
            GatewayKind::Context,
            GatewayKind::Output,
            GatewayKind::State,
        ],
        AuthorityMode::ExecuteWithRuntimeAuthority,
        session_id,
        request_id,
        trace_id,
        payload,
    );
    envelope.parent_event_id = parent_event_id;
    envelope.provenance = provenance;
    envelope
}

fn run_tool_command(
    mut command: Vec<String>,
    raw_dir: PathBuf,
    max_summary_bytes: usize,
) -> Result<CommandSummary> {
    if command.first().map(|s| s == "--").unwrap_or(false) {
        command.remove(0);
    }
    run_command(&command, None, raw_dir, max_summary_bytes)
}

fn execute_context_gateway(
    path: PathBuf,
    scope: String,
    compactness: String,
    diagnostics: String,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
) -> Result<()> {
    let request_id =
        request_id.unwrap_or_else(|| stable_id(&format!("context:{}:{scope}", path.display())));
    let trace_id = trace_id.unwrap_or_else(|| request_id.clone());
    let exp = expand_scope(&path, &scope, &compactness)?;
    let mut symbols = BTreeMap::new();
    symbols.extend(exp.symbol_map.symbols.clone());
    let decision = decide_context_need(&exp.compact_code, &symbols, &diagnostics);
    let context_ref = stable_id(&format!(
        "{}:{}:{}:{}:{}",
        exp.scope.path, exp.scope.start_line, exp.scope.end_line, compactness, exp.compact_code
    ));
    let full = if matches!(decision.action, FallbackAction::Full) {
        Some(serde_json::to_value(full_scope(&path, &scope)?)?)
    } else {
        None
    };
    let compact = if full.is_none() {
        Some(serde_json::to_value(&exp)?)
    } else {
        None
    };
    let provenance = ProvenanceRefs {
        context_refs: vec![context_ref.clone()],
        validation_status: Some(match decision.confidence {
            Confidence::High => ValidationStatus::Valid,
            Confidence::Medium => ValidationStatus::NotValidated,
            Confidence::Low => ValidationStatus::NonAuthoritative,
        }),
        ..Default::default()
    };
    let response = response_envelope(
        GatewayResponse::Context {
            action: format!("{:?}", decision.action).to_ascii_lowercase(),
            reason: decision.reason,
            compact_context: compact,
            full_context: full,
            context_ref,
        },
        &session_id,
        &request_id,
        &trace_id,
        None,
        provenance,
    );
    print_json(&response)
}

fn execute_output_gateway(
    payload: Option<PathBuf>,
    apply: bool,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
    parent_event_id: Option<String>,
) -> Result<()> {
    let request_id = request_id.unwrap_or_else(|| stable_id("output"));
    let trace_id = trace_id.unwrap_or_else(|| request_id.clone());
    if apply {
        bail!("output-gateway apply requires explicit authority/provenance implementation; current command is preview/validate only");
    }
    let text = read_payload(payload)?;
    let restore_body: RestorePayload = serde_json::from_str(&text)?;
    let restored = restore_payload(restore_body)?;
    let patch_ref = stable_id(&restored.restored_code);
    let preview_diff = format!("--- compact\n+++ restored\n@@\n{}", restored.restored_code);
    let provenance_status = if parent_event_id.is_some() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::NonAuthoritative
    };
    let provenance = ProvenanceRefs {
        patch_refs: vec![patch_ref.clone()],
        validation_status: Some(provenance_status.clone()),
        ..Default::default()
    };
    let response = response_envelope(
        GatewayResponse::Output {
            restored_code: restored.restored_code,
            preview_diff,
            validation_status: provenance_status,
            applied: false,
            patch_ref,
        },
        &session_id,
        &request_id,
        &trace_id,
        parent_event_id,
        provenance,
    );
    print_json(&response)
}

fn context_decision_from_value(v: &serde_json::Value) -> Result<ContextDecision> {
    let diagnostics = v.get("diagnostics").and_then(|x| x.as_str()).unwrap_or("");
    let compact_code = v
        .get("compact_code")
        .or_else(|| v.get("code"))
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let known_symbols: BTreeMap<String, String> = v
        .get("symbols")
        .or_else(|| v.get("known_symbols"))
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    Ok(decide_context_need(
        compact_code,
        &known_symbols,
        diagnostics,
    ))
}

fn read_payload(path: Option<PathBuf>) -> Result<String> {
    Ok(if let Some(p) = path {
        std::fs::read_to_string(p)?
    } else {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
        s
    })
}
fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn parse_gateway(value: &str) -> Result<GatewayKind> {
    match value.to_ascii_lowercase().as_str() {
        "tool" => Ok(GatewayKind::Tool),
        "context" => Ok(GatewayKind::Context),
        "output" => Ok(GatewayKind::Output),
        "state" => Ok(GatewayKind::State),
        _ => bail!("unknown gateway: {value}"),
    }
}

fn parse_output_mode(value: &str) -> Result<OutputMode> {
    match value.to_ascii_lowercase().as_str() {
        "text" => Ok(OutputMode::Text),
        "json" => Ok(OutputMode::Json),
        "jsonl" => Ok(OutputMode::Jsonl),
        "mcp_resource" | "mcp-resource" => Ok(OutputMode::McpResource),
        "provider_payload" | "provider-payload" => Ok(OutputMode::ProviderPayload),
        _ => bail!("unknown output mode: {value}"),
    }
}

fn stable_id(input: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("tfy_{:016x}", hasher.finish())
}
