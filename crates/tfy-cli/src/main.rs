use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
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
enum Cmd {
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
        #[arg(long, default_value_t = 1_000_000)]
        max_output_bytes: usize,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Tool Gateway entrypoint for agent runtimes that proxy ordinary commands through TFY.
    ToolGateway {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value_t = 1_000_000)]
        max_output_bytes: usize,
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
        #[arg(long, default_value_t = 1_000_000)]
        max_output_bytes: usize,
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
            max_output_bytes,
            command,
        } => execute_plain_tool_gateway(command, raw_dir, max_output_bytes)?,
        Cmd::ToolGateway {
            raw_dir,
            max_output_bytes,
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
            max_output_bytes,
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
            max_output_bytes,
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
        } => print!(
            "{}",
            raw_output(raw_dir, &raw_ref, around.as_deref(), context)?
        ),
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

fn execute_plain_tool_gateway(
    command: Vec<String>,
    raw_dir: PathBuf,
    max_output_bytes: usize,
) -> Result<()> {
    let summary = run_tool_command(command, raw_dir, max_output_bytes)?;
    print!("{}", summary.summary);
    std::process::exit(summary.exit_code);
}

#[allow(clippy::too_many_arguments)]
fn execute_structured_tool_gateway(
    command: Vec<String>,
    raw_dir: PathBuf,
    max_output_bytes: usize,
    json: bool,
    jsonl: bool,
    ledger: PathBuf,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
    parent_event_id: Option<String>,
) -> Result<()> {
    let summary = run_tool_command(command, raw_dir, max_output_bytes)?;
    let request_id = request_id
        .unwrap_or_else(|| stable_id(&format!("tool:{}:{}", summary.command, summary.raw_ref)));
    let trace_id = trace_id.unwrap_or_else(|| request_id.clone());
    let mut provenance = ProvenanceRefs {
        raw_refs: vec![summary.raw_ref.clone()],
        validation_status: Some(ValidationStatus::Valid),
        ..Default::default()
    };
    let response = response_envelope(
        GatewayResponse::ToolCommand {
            command: summary.command.clone(),
            exit_code: summary.exit_code,
            risk: summary.risk.clone(),
            summary: summary.summary.clone(),
            raw_ref: summary.raw_ref.clone(),
            evidence: summary.evidence.clone(),
        },
        &session_id,
        &request_id,
        &trace_id,
        parent_event_id.clone(),
        provenance.clone(),
    );
    provenance.source_event_ids = vec![request_id.clone()];
    let event = event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: summary.command,
            exit_code: summary.exit_code,
            risk: summary.risk,
            raw_ref: summary.raw_ref,
            summary_chars: summary.summary_chars,
        },
        &session_id,
        &request_id,
        &trace_id,
        parent_event_id,
        provenance,
    );
    append_event(ledger, &event)?;
    if jsonl {
        println!("{}", serde_json::to_string(&event)?);
        println!("{}", serde_json::to_string(&response)?);
    } else if json {
        print_json(&response)?;
    } else {
        if let GatewayResponse::ToolCommand { summary, .. } = &response.payload {
            print!("{}", summary);
        }
    }
    std::process::exit(match &response.payload {
        GatewayResponse::ToolCommand { exit_code, .. } => *exit_code,
        _ => 1,
    });
}

fn run_tool_command(
    mut command: Vec<String>,
    raw_dir: PathBuf,
    max_output_bytes: usize,
) -> Result<CommandSummary> {
    if command.first().map(|s| s == "--").unwrap_or(false) {
        command.remove(0);
    }
    run_command(&command, None, raw_dir, max_output_bytes)
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
