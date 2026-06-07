mod adapter;
mod gateways;
mod mcp;
mod util;

use adapter::{execute_adapter, AdapterCmd};
use anyhow::Result;
use clap::{Parser, Subcommand};
use gateways::{
    context_decision_from_value, execute_context_gateway, execute_output_gateway,
    execute_plain_tool_gateway, execute_structured_tool_gateway,
};
use mcp::{execute_mcp, McpCmd};
use std::io::{self, Write};
use std::path::PathBuf;
use tfy_core::*;
use tfy_runtime::*;
use util::{parse_gateway, parse_output_mode, print_json, read_payload, stable_id};

#[derive(Parser)]
#[command(name = "tfy", about = "Token-efficient AI work interface")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
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
        #[arg(long = "context-proof")]
        context_proof: Option<PathBuf>,
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
            context_proof,
            session_id,
            request_id,
            trace_id,
            parent_event_id,
        } => execute_output_gateway(
            payload,
            apply,
            context_proof,
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
