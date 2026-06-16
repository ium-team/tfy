mod adapter;
mod agent;
mod display;
mod gateways;
mod hook;
mod human;
mod mcp;
mod product;
mod util;
mod workspace;

use adapter::{execute_adapter, AdapterCmd};
use agent::{execute_agent, AgentCmd};
use anyhow::Result;
use clap::{Parser, Subcommand};
use display::{
    execute_restore_display, execute_restore_file, execute_restore_patch, RestoreDisplayCmd,
    RestoreFileCmd, RestorePatchCmd,
};
use gateways::{
    context_decision_from_value, execute_context_gateway, execute_output_gateway,
    execute_plain_tool_gateway, execute_structured_tool_gateway,
    execute_structured_tool_gateway_with_origin,
};
use hook::{execute_hook, HookCmd};
use human::{execute_human, HumanCmd};
use mcp::{execute_mcp, McpCmd};
use product::{
    execute_bench, execute_doctor, execute_explain, execute_fuckyou, execute_gain, execute_global,
    execute_init, execute_launch_report, execute_raw, execute_setup, execute_smoke, execute_start,
    execute_status, execute_stop, execute_use, BenchCmd, DoctorCmd, ExplainCmd, FuckyouCmd,
    GainCmd, GlobalCmd, InitCmd, LaunchReportCmd, RawCmd, SetupCmd, SmokeCmd, StartCmd, StatusCmd,
    StopCmd, UseCmd,
};
use std::path::PathBuf;
use tfy_core::*;
use tfy_runtime::*;
use util::{parse_gateway, parse_output_mode, print_json, read_payload, stable_id};
use workspace::{execute_workspace, WorkspaceCmd};

#[derive(Parser)]
#[command(name = "tfy", about = "Token-efficient AI work interface")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Explicit AI-agent runtime wrapper. Routes AI-originated I/O through TFY without touching normal terminals.
    Agent {
        #[command(subcommand)]
        cmd: AgentCmd,
    },
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
    /// Optional official-host hook shims. Thin routers to shared TFY gateways, disabled unless configured.
    Hook {
        #[command(subcommand)]
        cmd: HookCmd,
    },
    /// Explicit TFY-managed human shell/session commands. Does not globally intercept ordinary terminals.
    Human {
        #[command(subcommand)]
        cmd: HumanCmd,
    },
    /// Product-facing setup lifecycle for Codex/MCP guidance. Bare `tfy init` is a safe dry-run.
    Init(InitCmd),
    /// Diagnose local TFY and optional Codex-facing integration readiness.
    Doctor(DoctorCmd),
    /// Run local MCP smoke tests or print host-facing smoke checklists.
    Smoke(SmokeCmd),
    /// Report measured TFY savings from adapter/MCP ledgers.
    Gain(GainCmd),
    /// Easy setup for supported AI-agent host routing; never intercepts ordinary terminals.
    Setup(SetupCmd),
    /// Show which TFY routing/apply/restore surfaces are active, configured, or out of scope.
    Status(StatusCmd),
    /// Explain the compact-AI/readable-file TFY model in user-facing terms.
    Explain(ExplainCmd),
    /// Report launch-readiness evidence, blockers, host readiness matrix, and measured savings.
    LaunchReport(LaunchReportCmd),
    /// Generate deterministic benchmark manifests and optional RTK-safe comparator results.
    Bench(BenchCmd),
    /// Start TFY lifecycle intent in this project. Bare interactive command opens a target TUI wizard.
    Start(StartCmd),
    /// Stop TFY lifecycle intent in this project without deleting raw evidence. Bare interactive command opens a target TUI wizard.
    Stop(StopCmd),
    /// Scoped TFY-owned lifecycle cleanup in this project, confirmation-gated. Bare interactive command opens target and confirmation TUI wizards.
    Fuckyou(FuckyouCmd),
    /// Manage user-global TFY lifecycle intent.
    Global {
        #[command(subcommand)]
        cmd: GlobalCmd,
    },
    /// Convenience aliases for user-global default TFY lifecycle intent.
    Use {
        #[command(subcommand)]
        cmd: UseCmd,
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
    /// Restore compact code into human-readable display text. Display-only, not apply authority.
    RestoreDisplay(RestoreDisplayCmd),
    /// Restore compact AI transport into canonical readable file code, optionally writing it.
    RestoreFile(RestoreFileCmd),
    /// Restore compact AI patch/output into canonical readable patch/code before validation/apply.
    RestorePatch(RestorePatchCmd),
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
    /// Workspace-level validate/apply plan for multi-file changes with plan hash + per-op proofs.
    Workspace {
        #[command(subcommand)]
        cmd: WorkspaceCmd,
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
    /// Recover, inspect, export, or prune locally stored raw command evidence.
    Raw(RawCmd),
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
        Cmd::Agent { cmd } => execute_agent(cmd)?,
        Cmd::Adapter { cmd } => execute_adapter(cmd)?,
        Cmd::Mcp { cmd } => execute_mcp(cmd)?,
        Cmd::Hook { cmd } => execute_hook(cmd)?,
        Cmd::Human { cmd } => execute_human(cmd)?,
        Cmd::Init(cmd) => execute_init(cmd)?,
        Cmd::Doctor(cmd) => execute_doctor(cmd)?,
        Cmd::Smoke(cmd) => execute_smoke(cmd)?,
        Cmd::Gain(cmd) => execute_gain(cmd)?,
        Cmd::LaunchReport(cmd) => execute_launch_report(cmd)?,
        Cmd::Bench(cmd) => execute_bench(cmd)?,
        Cmd::Start(cmd) => execute_start(cmd)?,
        Cmd::Stop(cmd) => execute_stop(cmd)?,
        Cmd::Fuckyou(cmd) => execute_fuckyou(cmd)?,
        Cmd::Global { cmd } => execute_global(cmd)?,
        Cmd::Use { cmd } => execute_use(cmd)?,
        Cmd::Setup(cmd) => execute_setup(cmd)?,
        Cmd::Status(cmd) => execute_status(cmd)?,
        Cmd::Explain(cmd) => execute_explain(cmd)?,
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
        Cmd::RestoreDisplay(cmd) => execute_restore_display(cmd)?,
        Cmd::RestoreFile(cmd) => execute_restore_file(cmd)?,
        Cmd::RestorePatch(cmd) => execute_restore_patch(cmd)?,
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
        Cmd::Shell {
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
        } => execute_structured_tool_gateway_with_origin(
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
            AdapterKind::Shell,
            Origin::agent_runtime(OriginHost::Generic, OriginInvocation::Wrapper),
            RouteEvidence::generic_shell_adapter(),
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
        Cmd::Workspace { cmd } => execute_workspace(cmd)?,
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
        Cmd::Raw(cmd) => execute_raw(cmd)?,
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
