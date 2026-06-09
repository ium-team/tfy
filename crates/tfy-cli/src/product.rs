use crate::util::{print_json, stable_id};
use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tfy_runtime::{load_events, AdapterKind, GatewayEvent, OriginInvocation};

const TFY_CODEX_START: &str = "<!-- TFY:CODEX:START -->";
const TFY_CODEX_END: &str = "<!-- TFY:CODEX:END -->";
const NO_DATA_MESSAGE: &str = "No TFY savings data found yet";
const REQUIRED_MCP_TOOLS: &[&str] = &[
    "tfy_tool_run",
    "tfy_scope_list",
    "tfy_context_get",
    "tfy_output_validate",
    "tfy_output_apply",
    "tfy_restore_display",
    "tfy_workspace_validate",
    "tfy_workspace_apply",
    "tfy_state_project",
    "tfy_adapter_report",
];

#[derive(Args, Clone)]
pub(crate) struct InitCmd {
    #[arg(long)]
    pub codex: bool,
    #[arg(long)]
    pub show: bool,
    #[arg(long)]
    pub uninstall: bool,
    #[arg(long)]
    pub project: bool,
    #[arg(long)]
    pub global: bool,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub apply: bool,
    #[arg(long, default_value = "local-session")]
    pub session: String,
}

#[derive(Args, Clone)]
pub(crate) struct DoctorCmd {
    #[arg(long)]
    pub codex: bool,
    /// Named AI-agent host to diagnose (codex, claude-code, cursor, opencode, hermes, openclaw).
    #[arg(long = "host")]
    pub host: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct SmokeCmd {
    #[arg(long)]
    pub mcp: bool,
    #[arg(long)]
    pub local: bool,
    #[arg(long)]
    pub codex: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub all: bool,
    /// Named AI-agent host checklist/smoke target.
    #[arg(long = "host")]
    pub host: Vec<String>,
}

#[derive(Args, Clone)]
pub(crate) struct GainCmd {
    #[arg(long)]
    pub ledger: Vec<PathBuf>,
    #[arg(long, default_value = "local-session")]
    pub session: String,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct SetupCmd {
    /// Prepare TFY for supported AI-agent host routing. Does not touch ordinary terminals.
    #[arg(long)]
    pub ai: bool,
    /// Print Codex MCP/instruction setup guidance.
    #[arg(long)]
    pub codex: bool,
    /// Named AI-agent host to configure (codex, claude-code, cursor, opencode, hermes, openclaw).
    #[arg(long = "host")]
    pub host: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub apply: bool,
    /// Remove TFY-owned host config where safe reversible uninstall is implemented.
    #[arg(long)]
    pub uninstall: bool,
    /// Use project-local host config when supported.
    #[arg(long)]
    pub project: bool,
    /// Use user-global host config when supported.
    #[arg(long)]
    pub global: bool,
    #[arg(long, default_value = "local-session")]
    pub session: String,
}

#[derive(Args, Clone)]
pub(crate) struct StatusCmd {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct ExplainCmd {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct LaunchReportCmd {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub ledger: Vec<PathBuf>,
    /// JSON evidence proving host setup and real host invocation per route.
    #[arg(long = "host-evidence")]
    pub host_evidence: Vec<PathBuf>,
    #[arg(long, default_value = "local-session")]
    pub session: String,
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeTarget {
    Project,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostConfigScope {
    Project,
    Global,
}

impl HostConfigScope {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

impl ScopeTarget {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }
    fn path(self) -> PathBuf {
        match self {
            Self::Project => PathBuf::from("AGENTS.md"),
            Self::Global => home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".codex")
                .join("AGENTS.md"),
        }
    }
}

#[derive(Serialize)]
struct InitStatus {
    action: String,
    target: String,
    dry_run: bool,
    applied: bool,
    path: String,
    installed: bool,
    tiers: Vec<&'static str>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct Diagnostic {
    name: String,
    status: String,
    message: String,
}

#[derive(Serialize)]
struct DoctorReport {
    status: String,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize)]
struct SurfaceStatus {
    name: String,
    status: String,
    message: String,
}

#[derive(Serialize)]
struct ProductStatusReport {
    status: String,
    minimum_v1_host_matrix: Vec<HostReadiness>,
    surfaces: Vec<SurfaceStatus>,
    claim_evidence_ladder: Vec<String>,
    launch_claim_gate: String,
    not_supported: Vec<String>,
    truthfulness_boundary: String,
}

#[derive(Serialize)]
struct HostReadiness {
    host: String,
    status: String,
    required_for_v1: bool,
    claim_tier: String,
    evidence_tiers: Vec<String>,
    next_evidence_tier: String,
    supported_ingress: Vec<String>,
    equivalence_ingress: Vec<String>,
    unsupported_ingress: Vec<String>,
    config_strategy: String,
    apply_strategy: String,
    smoke_strategy: String,
    host_evidence_strategy: String,
    setup: String,
    normal_workflow: String,
    evidence_gate: Vec<String>,
    launch_claim: String,
}

#[derive(Clone)]
struct HostIntegration {
    id: &'static str,
    display: &'static str,
    status: &'static str,
    required_for_v1: bool,
    config: &'static str,
    transport: &'static str,
    official_source: &'static str,
    config_strategy: &'static str,
    apply_strategy: &'static str,
    smoke_strategy: &'static str,
    host_evidence_strategy: &'static str,
    setup: &'static str,
    normal_workflow: &'static str,
    launch_claim: &'static str,
    evidence_gate: &'static [&'static str],
}

#[derive(Serialize)]
struct LaunchReadinessReport {
    status: String,
    host_matrix: Vec<HostReadiness>,
    claim_evidence_ladder: Vec<String>,
    gain: GainReport,
    host_evidence: HostEvidenceSummary,
    release_thresholds: ReleaseThresholds,
    measurement_method: MeasurementMethod,
    privacy_raw_store: PrivacyRawStorePolicy,
    overhead_policy: OverheadPolicy,
    unsupported_claim_audit: UnsupportedClaimAudit,
    blockers: Vec<String>,
    not_supported: Vec<String>,
    required_benchmark_scenarios: Vec<String>,
}

#[derive(Serialize)]
struct ProductExplainReport {
    ai_transport: String,
    file_and_user_output: String,
    automatic_routing: String,
    apply_model: String,
    what_tfy_changed: Vec<String>,
    data_stored_locally: Vec<String>,
    raw_recovery: String,
    deletion_export: String,
    launch_pass_block_reason: String,
    out_of_scope: Vec<String>,
}

#[derive(Serialize)]
struct SmokeReport {
    status: String,
    mode: String,
    sample_path: String,
    scope_id: String,
    preview_applied: bool,
    apply_applied: bool,
    ledger_events: usize,
    evidence: Vec<String>,
}

#[derive(Serialize)]
struct SmokeSuiteReport {
    status: String,
    mode: String,
    reports: Vec<SmokeReport>,
    evidence: Vec<String>,
}

#[derive(Default, Serialize)]
struct GainReport {
    status: String,
    sessions: Vec<String>,
    ledgers: Vec<String>,
    commands: usize,
    failures: usize,
    raw_bytes: usize,
    model_bytes: usize,
    saved_bytes: isize,
    estimated_raw_tokens: usize,
    estimated_model_tokens: usize,
    estimated_saved_tokens: isize,
    savings_pct: f64,
    fallback_frequency: f64,
    rendering_counts: BTreeMap<String, usize>,
    family_counts: BTreeMap<String, usize>,
    measurement_method: MeasurementMethod,
    message: Option<String>,
}

#[derive(Clone, Serialize)]
struct MeasurementMethod {
    byte_savings: String,
    token_estimate: String,
    tokenizer_exact: bool,
}

impl Default for MeasurementMethod {
    fn default() -> Self {
        Self {
            byte_savings: "exact UTF-8 byte counts recorded from raw/model-visible gateway payloads".into(),
            token_estimate: "conservative proxy estimate using ceil(bytes/4) when tokenizer-specific counts are unavailable".into(),
            tokenizer_exact: false,
        }
    }
}

#[derive(Clone, Serialize)]
struct ReleaseThresholds {
    correctness_parity: String,
    default_savings: String,
    positive_required_route_savings: String,
    missed_context_or_evidence: String,
    overhead: String,
    time_to_first_saving: String,
}

impl Default for ReleaseThresholds {
    fn default() -> Self {
        Self {
            correctness_parity: "100% mandatory benchmark scenarios pass required checks".into(),
            default_savings: "no negative model-visible bytes/tokens unless an explicit fallback/debug/internal reason is recorded".into(),
            positive_required_route_savings: "at least one command-heavy or repeated-output scenario per required route shows positive saved bytes/tokens".into(),
            missed_context_or_evidence: "0 unhandled missed-context or missed-evidence incidents".into(),
            overhead: "report local overhead; >2x baseline or +5s requires a documented launch-report exception".into(),
            time_to_first_saving: "prepared-repo quickstart reaches first positive gain/launch-report evidence in under 10 minutes".into(),
        }
    }
}

#[derive(Clone, Serialize)]
struct PrivacyRawStorePolicy {
    default_raw_dirs: Vec<String>,
    retention_default: String,
    deletion_export: String,
    disclosure: String,
    blockers: Vec<String>,
}

impl Default for PrivacyRawStorePolicy {
    fn default() -> Self {
        Self {
            default_raw_dirs: vec![".tfy/raw".into(), ".tfy/mcp/raw or configured --raw-dir".into()],
            retention_default: "local project data is retained until the user deletes/prunes the .tfy directory; TFY does not upload raw evidence".into(),
            deletion_export: "delete/export by inspecting .tfy/raw, .tfy/mcp/ledger.jsonl, .tfy/adapter/ledger.jsonl, or configured raw/ledger paths; release implementation must add first-class commands before claiming managed retention".into(),
            disclosure: "TFY stores raw command/context evidence locally before compacting model-visible text so correctness and audit recovery remain possible".into(),
            blockers: vec![
                "secret leakage in CLI/log/ledger/launch-report output".into(),
                "path traversal or symlink escape in raw-store paths".into(),
                "unsafe raw-store permissions".into(),
            ],
        }
    }
}

#[derive(Clone, Serialize)]
struct OverheadPolicy {
    measurement: String,
    exception_threshold: String,
    exception_authority: String,
    report_location: String,
}

impl Default for OverheadPolicy {
    fn default() -> Self {
        Self {
            measurement:
                "local smoke/benchmark wall time must be reported alongside launch evidence".into(),
            exception_threshold: ">2x baseline or +5s local overhead".into(),
            exception_authority:
                "release owner must record an explicit launch-report exception before release"
                    .into(),
            report_location: "tfy launch-report --json overhead_policy and blockers".into(),
        }
    }
}

#[derive(Default, Serialize)]
struct HostEvidenceSummary {
    commands: usize,
    raw_refs: usize,
    no_negative_savings: bool,
    positive_savings: bool,
    generic_shell_route: RouteEvidence,
    tfy_agent_adapter_route: RouteEvidence,
    mcp_stdio_route: RouteEvidence,
    generic_shell_wrapper: bool,
    tfy_agent_adapter: bool,
    mcp_tool: bool,
    mcp_context: bool,
    mcp_output: bool,
    mcp_state: bool,
    codex_real_invocation: bool,
    named_hosts: BTreeMap<String, RouteEvidence>,
    evidence_notes: Vec<String>,
}

#[derive(Default, Clone, Serialize)]
struct RouteEvidence {
    commands: usize,
    raw_refs: usize,
    no_negative_savings: bool,
    positive_savings: bool,
    setup_verified: bool,
    real_invocation_verified: bool,
    setup_artifact_verified: bool,
    invocation_artifact_verified: bool,
    overhead_measured: bool,
    overhead_passed: bool,
    overhead_ms: Option<u64>,
    baseline_ms: Option<u64>,
    overhead_exception: Option<String>,
    tool: bool,
    context: bool,
    output: bool,
    state: bool,
    host_bound_evidence: bool,
    official_docs_backed: bool,
    kill_switch_available: bool,
    uninstall_available: bool,
    route_type: Option<String>,
    config_scope: Option<String>,
    config_path: Option<String>,
    smoke_id: Option<String>,
    host_version: Option<String>,
}

#[derive(Deserialize)]
struct HostEvidenceFile {
    hosts: Vec<HostSetupEvidence>,
}

#[derive(Deserialize)]
struct HostSetupEvidence {
    host: String,
    setup_verified: bool,
    real_invocation_verified: bool,
    setup_artifact: Option<PathBuf>,
    invocation_artifact: Option<PathBuf>,
    overhead_ms: Option<u64>,
    baseline_ms: Option<u64>,
    overhead_exception: Option<String>,
    host_id: Option<String>,
    host_version: Option<String>,
    config_scope: Option<String>,
    config_path: Option<PathBuf>,
    route_type: Option<String>,
    ledger_artifact: Option<PathBuf>,
    raw_artifact: Option<PathBuf>,
    redacted_public_bytes: Option<usize>,
    model_visible_bytes: Option<usize>,
    timestamp: Option<String>,
    smoke_id: Option<String>,
    official_docs_backed: Option<bool>,
    kill_switch_available: Option<bool>,
    uninstall_available: Option<bool>,
}

#[derive(Serialize)]
struct UnsupportedClaimAudit {
    status: String,
    audited_claims: Vec<String>,
    rule: String,
}

pub(crate) fn execute_init(cmd: InitCmd) -> Result<()> {
    let targets = selected_targets(cmd.project, cmd.global);
    if cmd.show {
        let statuses: Vec<_> = targets.iter().map(|target| init_status(*target)).collect();
        print_json(&json!({"action":"show","targets":statuses}))?;
        return Ok(());
    }
    let dry_run = !cmd.apply || cmd.dry_run;
    let mut results = Vec::new();
    for target in targets {
        if cmd.uninstall {
            results.push(uninstall_codex(target, dry_run)?);
        } else {
            results.push(install_codex(target, &cmd.session, dry_run)?);
        }
    }
    print_init_results(&results, &cmd.session);
    Ok(())
}

pub(crate) fn execute_doctor(cmd: DoctorCmd) -> Result<()> {
    let report = build_doctor_report(cmd.codex || !cmd.host.is_empty());
    let host_reports: Vec<_> = cmd
        .host
        .iter()
        .map(|host| host_doctor_report(host))
        .collect::<Result<_>>()?;
    if cmd.json {
        if host_reports.is_empty() {
            print_json(&report)?;
        } else {
            print_json(&json!({
                "status": report.status,
                "diagnostics": report.diagnostics,
                "hosts": host_reports
            }))?;
        }
    } else {
        println!("TFY doctor: {}", report.status);
        for d in &report.diagnostics {
            println!("{} {} — {}", status_icon(&d.status), d.name, d.message);
        }
        for host in &host_reports {
            println!(
                "host {}: {} — {}",
                host["host"], host["status"], host["message"]
            );
        }
    }
    if report.status == "fail" {
        bail!("tfy doctor found failing local checks");
    }
    Ok(())
}

pub(crate) fn execute_smoke(cmd: SmokeCmd) -> Result<()> {
    if !cmd.host.is_empty() {
        let host_reports: Vec<_> = cmd
            .host
            .iter()
            .map(|host| host_smoke_report(host))
            .collect::<Result<_>>()?;
        if cmd.json {
            print_json(&json!({
                "status": if host_reports.iter().all(|h| h["status"] != "unsupported") {"pass"} else {"warn"},
                "mode": "host",
                "hosts": host_reports
            }))?;
        } else {
            for host in host_reports {
                println!(
                    "TFY smoke host {}: {} — {}",
                    host["host"], host["status"], host["message"]
                );
            }
        }
        return Ok(());
    }
    if cmd.all {
        let mut reports = Vec::new();
        let mut evidence = Vec::new();
        let adapter = run_adapter_smoke()?;
        evidence.extend(adapter.evidence.clone());
        reports.push(adapter);
        let agent = run_agent_smoke()?;
        evidence.extend(agent.evidence.clone());
        reports.push(agent);
        let mcp = run_mcp_smoke()?;
        evidence.extend(mcp.evidence.clone());
        reports.push(mcp);
        if cmd.codex {
            if cmd.json {
                write_codex_smoke_checklist(std::io::stderr())?;
            } else {
                print_codex_smoke_checklist()?;
            }
            evidence.push(
                "codex smoke remains checklist-only until real host invocation evidence exists"
                    .into(),
            );
        }
        let suite = SmokeSuiteReport {
            status: if reports.iter().all(|r| r.status == "pass") {
                "pass"
            } else {
                "fail"
            }
            .into(),
            mode: "all".into(),
            reports,
            evidence,
        };
        if cmd.json {
            print_json(&suite)?;
        } else {
            println!("TFY smoke all: {}", suite.status);
            for e in &suite.evidence {
                println!("✓ {e}");
            }
        }
        return Ok(());
    }
    if cmd.codex {
        if cmd.json && (cmd.mcp || cmd.local) {
            write_codex_smoke_checklist(std::io::stderr())?;
        } else {
            print_codex_smoke_checklist()?;
        }
        if !(cmd.mcp || cmd.local) {
            return Ok(());
        }
    }
    let report = run_mcp_smoke()?;
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("TFY smoke {}: {}", report.mode, report.status);
        for e in &report.evidence {
            println!("✓ {e}");
        }
    }
    Ok(())
}

pub(crate) fn execute_setup(cmd: SetupCmd) -> Result<()> {
    if !(cmd.ai || cmd.codex || !cmd.host.is_empty()) {
        bail!("setup requires --ai, --codex, and/or --host <host>; use `tfy setup --ai --host codex --dry-run`");
    }
    if cmd.ai {
        println!("TFY setup ai: supported_host_routing=true ordinary_terminal_interception=false provider_gateway=false editor_integration=false");
        println!("AI reads compact transport through TFY context/tool/MCP surfaces; files and user output are restored to readable canonical code.");
    }
    if cmd.codex {
        execute_init(InitCmd {
            codex: true,
            show: false,
            uninstall: cmd.uninstall,
            project: cmd.project,
            global: cmd.global,
            dry_run: cmd.dry_run || !cmd.apply,
            apply: cmd.apply,
            session: cmd.session.clone(),
        })?;
    }
    for host in &cmd.host {
        let integration = host_integration(host)?;
        let scope = if cmd.global {
            HostConfigScope::Global
        } else {
            HostConfigScope::Project
        };
        print_host_setup(
            integration,
            &cmd.session,
            cmd.dry_run || !cmd.apply,
            cmd.apply,
            cmd.uninstall,
            scope,
        )?;
    }
    Ok(())
}

pub(crate) fn execute_status(cmd: StatusCmd) -> Result<()> {
    let report = build_product_status_report();
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("TFY status: {}", report.status);
        for host in &report.minimum_v1_host_matrix {
            println!(
                "host {}: {} — {}",
                host.host, host.status, host.launch_claim
            );
        }
        for surface in &report.surfaces {
            println!("{}: {} — {}", surface.name, surface.status, surface.message);
        }
        println!("not_supported: {}", report.not_supported.join(", "));
    }
    Ok(())
}

pub(crate) fn execute_explain(cmd: ExplainCmd) -> Result<()> {
    let report = build_explain_report();
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("AI transport: {}", report.ai_transport);
        println!("Files/user output: {}", report.file_and_user_output);
        println!("Automatic routing: {}", report.automatic_routing);
        println!("Apply: {}", report.apply_model);
        println!("What changed: {}", report.what_tfy_changed.join("; "));
        println!(
            "Data stored locally: {}",
            report.data_stored_locally.join("; ")
        );
        println!("Raw recovery: {}", report.raw_recovery);
        println!("Deletion/export: {}", report.deletion_export);
        println!("Launch pass/block: {}", report.launch_pass_block_reason);
        println!("Out of scope: {}", report.out_of_scope.join(", "));
    }
    Ok(())
}

pub(crate) fn execute_launch_report(cmd: LaunchReportCmd) -> Result<()> {
    let ledgers = gain_ledgers(&cmd.ledger);
    let session = if cmd.all {
        None
    } else {
        Some(cmd.session.as_str())
    };
    let gain = build_gain_report(&ledgers, session)?;
    let host_evidence = build_host_evidence_summary(&ledgers, session, &cmd.host_evidence);
    let status = build_product_status_report();
    let report = build_launch_readiness_report(status, gain, host_evidence);
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("TFY launch readiness: {}", report.status);
        println!(
            "host_matrix={}",
            report
                .host_matrix
                .iter()
                .map(|h| format!("{}:{}", h.host, h.status))
                .collect::<Vec<_>>()
                .join(",")
        );
        println!(
            "commands={} saved_bytes={} fallback_frequency={:.2}",
            report.gain.commands, report.gain.saved_bytes, report.gain.fallback_frequency
        );
        if !report.blockers.is_empty() {
            println!("blockers={}", report.blockers.join("; "));
        }
        println!("not_supported={}", report.not_supported.join(","));
    }
    Ok(())
}

pub(crate) fn execute_gain(cmd: GainCmd) -> Result<()> {
    let ledgers = gain_ledgers(&cmd.ledger);
    let report = build_gain_report(&ledgers, if cmd.all { None } else { Some(&cmd.session) })?;
    if cmd.json {
        print_json(&report)?;
    } else if report.commands == 0 {
        println!("{NO_DATA_MESSAGE}");
        println!("Hints: run `tfy adapter run -- <command>` or call MCP `tfy_tool_run` for command-output savings data.");
    } else {
        println!("TFY gain: {} command(s)", report.commands);
        println!("raw_bytes={}", report.raw_bytes);
        println!("model_bytes={}", report.model_bytes);
        println!("saved_bytes={}", report.saved_bytes);
        println!("estimated_saved_tokens={}", report.estimated_saved_tokens);
        println!("savings_pct={:.2}", report.savings_pct);
        println!("fallback_frequency={:.2}", report.fallback_frequency);
    }
    Ok(())
}

fn host_registry() -> Vec<HostIntegration> {
    vec![
        HostIntegration {
            id: "codex",
            display: "Codex",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "codex mcp add tfy -- tfy mcp serve ... or ~/.codex/config.toml [mcp_servers.tfy]",
            transport: "MCP stdio",
            official_source: "https://developers.openai.com/learn/docs-mcp",
            config_strategy: "Codex MCP command/TOML setup",
            apply_strategy: "project AGENTS.md apply via tfy init; Codex TOML remains dry-run/manual",
            smoke_strategy: "local MCP smoke plus host invocation artifact",
            host_evidence_strategy: "host-bound MCP ledger/raw evidence with Codex invocation artifact",
            setup: "project AGENTS.md guidance plus Codex MCP command/TOML snippet",
            normal_workflow: "Codex may call TFY MCP tools after host MCP routing is configured; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Codex invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official OpenAI MCP setup source pinned",
                "local TFY MCP initialize/tools-list smoke",
                "real Codex invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "claude-code",
            display: "Claude Code",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "claude mcp add ... or project .mcp.json mcpServers.tfy",
            transport: "MCP stdio/http per Claude support",
            official_source: "https://code.claude.com/docs/en/agent-sdk/mcp",
            config_strategy: "project .mcp.json mcpServers.tfy or claude mcp add",
            apply_strategy: "dry-run/manual until project .mcp.json writer is implemented",
            smoke_strategy: "Claude MCP list/invocation artifact when available; checklist otherwise",
            host_evidence_strategy: "host-bound MCP ledger/raw evidence with Claude invocation artifact",
            setup: "Claude MCP command and .mcp.json snippet; hooks are follow-up only when official/tested",
            normal_workflow: "Claude Code uses TFY only through configured MCP tools; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Claude invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official Claude MCP source pinned",
                "local TFY MCP initialize/tools-list smoke",
                "real Claude invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "cursor",
            display: "Cursor",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "~/.cursor/mcp.json or project .cursor/mcp.json mcpServers.tfy",
            transport: "MCP host routing",
            official_source: "https://docs.cursor.com/context/model-context-protocol",
            config_strategy: "project .cursor/mcp.json or global ~/.cursor/mcp.json mcpServers.tfy",
            apply_strategy: "safe JSON writer for project .cursor/mcp.json with backup/idempotency/uninstall",
            smoke_strategy: "Cursor MCP invocation artifact plus local MCP smoke; checklist until host CLI smoke is available",
            host_evidence_strategy: "host-bound MCP ledger/raw evidence with Cursor invocation artifact",
            setup: "Cursor ~/.cursor/mcp.json or project .cursor/mcp.json snippet plus rule/instruction guidance",
            normal_workflow: "Cursor agent can use TFY MCP tools after MCP server is enabled; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Cursor invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "Cursor/OpenAI MCP setup source pinned",
                "local TFY MCP initialize/tools-list smoke",
                "real Cursor invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "opencode",
            display: "OpenCode",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "opencode.json(c) mcp.tfy local server entry",
            transport: "MCP local server",
            official_source: "https://opencode.ai/docs/mcp-servers",
            config_strategy: "opencode.json(c) mcp local server entry",
            apply_strategy: "dry-run/manual until comment-preserving JSONC writer or CLI route is tested",
            smoke_strategy: "OpenCode MCP invocation artifact; checklist until automated route exists",
            host_evidence_strategy: "host-bound MCP ledger/raw evidence with OpenCode invocation artifact",
            setup: "OpenCode opencode.json(c) MCP snippet; JSONC apply remains dry-run until comment-preserving writer exists",
            normal_workflow: "OpenCode can discover TFY MCP tools from config; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real OpenCode invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official OpenCode MCP setup source pinned",
                "local TFY MCP initialize/tools-list smoke",
                "real OpenCode invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "hermes",
            display: "Hermes",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "~/.hermes/config.yaml mcp_servers.tfy or hermes mcp add tfy ... with tools.include filtering",
            transport: "MCP stdio/http per Hermes support",
            official_source: "https://hermes-agent.nousresearch.com/docs/user-guide/features/mcp",
            config_strategy: "~/.hermes/config.yaml mcp_servers or hermes mcp add",
            apply_strategy: "dry-run/manual until YAML writer or Hermes CLI route is tested",
            smoke_strategy: "Hermes MCP test/dashboard artifact plus local MCP smoke",
            host_evidence_strategy: "host-bound MCP ledger/raw evidence with Hermes invocation artifact",
            setup: "Nous Hermes mcp_servers YAML / hermes mcp add guidance with least-surface tool include list",
            normal_workflow: "Hermes discovers TFY MCP tools at startup/reload; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Hermes invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official Nous Hermes MCP source pinned",
                "local TFY MCP initialize/tools-list smoke",
                "real Hermes invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "openclaw",
            display: "OpenClaw",
            status: "planned_discovery",
            required_for_v1: false,
            config: "unknown; no TFY-consumable host route accepted yet",
            transport: "unknown",
            official_source: "none accepted yet",
            config_strategy: "none; official/current route not proven",
            apply_strategy: "unsupported",
            smoke_strategy: "planned discovery only",
            host_evidence_strategy: "ignored until route proof changes registry",
            setup: "no setup/apply/smoke support until official docs or installed CLI proof shows a route",
            normal_workflow: "discovery-only; TFY must not claim OpenClaw automatic routing",
            launch_claim: "unsupported/planned until official/current evidence proves a safe route",
            evidence_gate: &[
                "official docs or installed local CLI proof required",
                "route-specific setup and invocation artifacts required before support",
            ],
        },
    ]
}

fn host_integration(host: &str) -> Result<HostIntegration> {
    let normalized = host.trim().to_ascii_lowercase().replace('_', "-");
    host_registry()
        .into_iter()
        .find(|candidate| {
            candidate.id == normalized
                || (candidate.id == "claude-code" && normalized == "claude")
                || (candidate.id == "opencode" && normalized == "open-code")
        })
        .ok_or_else(|| {
            anyhow!(
                "unknown host '{host}'; supported hosts: {}",
                host_registry()
                    .iter()
                    .map(|h| h.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn host_mcp_args(session: &str) -> String {
    format!(
        "[\"mcp\", \"serve\", \"--session\", \"{session}\", \"--ledger\", \".tfy/mcp/ledger.jsonl\", \"--raw-dir\", \".tfy/raw\"]"
    )
}

fn host_setup_snippet(host: &HostIntegration, session: &str) -> String {
    let args = host_mcp_args(session);
    match host.id {
        "codex" => format!(
            "codex mcp add tfy -- tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw\n\n[mcp_servers.tfy]\ncommand = \"tfy\"\nargs = {args}\n"
        ),
        "claude-code" => format!(
            "claude mcp add tfy -- tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw\n\n{{\n  \"mcpServers\": {{\n    \"tfy\": {{\n      \"command\": \"tfy\",\n      \"args\": {args}\n    }}\n  }}\n}}\n"
        ),
        "cursor" => format!(
            "{{\n  \"mcpServers\": {{\n    \"tfy\": {{\n      \"command\": \"tfy\",\n      \"args\": {args}\n    }}\n  }}\n}}\n"
        ),
        "opencode" => format!(
            "{{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {{\n    \"tfy\": {{\n      \"type\": \"local\",\n      \"command\": [\"tfy\", \"mcp\", \"serve\", \"--session\", \"{session}\", \"--ledger\", \".tfy/mcp/ledger.jsonl\", \"--raw-dir\", \".tfy/raw\"],\n      \"enabled\": true\n    }}\n  }}\n}}\n"
        ),
        "hermes" => format!(
            "hermes mcp add tfy --command tfy --args mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw\n\nmcp_servers:\n  tfy:\n    command: \"tfy\"\n    args: [\"mcp\", \"serve\", \"--session\", \"{session}\", \"--ledger\", \".tfy/mcp/ledger.jsonl\", \"--raw-dir\", \".tfy/raw\"]\n    enabled: true\n    tools:\n      include: [tfy_tool_run, tfy_raw_get, tfy_scope_list, tfy_context_get, tfy_output_validate, tfy_output_apply, tfy_state_project, tfy_adapter_report]\n"
        ),
        "openclaw" => "OpenClaw is planned_discovery only; no TFY setup snippet is emitted until official/current evidence proves a safe route.\n".into(),
        _ => unreachable!("host registry returned unknown host"),
    }
}

fn print_host_setup(
    host: HostIntegration,
    session: &str,
    dry_run: bool,
    apply: bool,
    uninstall: bool,
    scope: HostConfigScope,
) -> Result<()> {
    if host.status == "planned_discovery" {
        println!("TFY setup host={} status=planned_discovery", host.id);
        println!("source={}", host.official_source);
        println!("{}", host.setup);
        println!("{}", host.launch_claim);
        return Ok(());
    }
    if (apply || uninstall) && host.id == "cursor" {
        let result = configure_cursor_project_mcp(session, scope, dry_run, uninstall)?;
        let (status, claim_tier) = if dry_run {
            ("dry_run", "configurable")
        } else if uninstall {
            ("removed_unverified", "applied_unverified")
        } else {
            ("applied_unverified", "applied_unverified")
        };
        println!("TFY setup host=cursor status={status} claim_tier={claim_tier}");
        println!("display={}", host.display);
        println!("transport={}", host.transport);
        println!("official_source={}", host.official_source);
        println!(
            "dry_run={} applied={} action={}",
            dry_run,
            !dry_run,
            if uninstall { "uninstall" } else { "install" }
        );
        println!("scope={}", result["scope"]);
        println!("config_path={}", result["config_path"]);
        println!("backup_path={}", result["backup_path"]);
        println!("setup_success_is_not_savings_success=true");
        println!("{}", host.launch_claim);
        return Ok(());
    }
    if apply || uninstall {
        bail!(
            "automatic --apply/--uninstall for host '{}' is not implemented safely yet; rerun with --dry-run and apply the printed reversible host config manually",
            host.id
        );
    }
    println!("TFY setup host={} status={}", host.id, host.status);
    println!("display={}", host.display);
    println!("transport={}", host.transport);
    println!("official_source={}", host.official_source);
    println!("config={}", host.config);
    println!("dry_run={dry_run} applied=false");
    println!("setup_success_is_not_savings_success=true");
    println!("--- snippet ---");
    print!("{}", host_setup_snippet(&host, session));
    println!("--- boundary ---");
    println!("MCP host routing only; no private hidden hooks, provider prompt mutation, editor auto-integration, or universal human-shell interception.");
    println!("{}", host.launch_claim);
    Ok(())
}

fn configure_cursor_project_mcp(
    session: &str,
    scope: HostConfigScope,
    dry_run: bool,
    uninstall: bool,
) -> Result<Value> {
    if scope == HostConfigScope::Global {
        bail!("cursor --global apply is not implemented safely yet; use --project for reversible .cursor/mcp.json writes");
    }
    let path = PathBuf::from(".cursor").join("mcp.json");
    let backup = cursor_backup_path(&path);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    if uninstall && existing.is_none() {
        return Ok(cursor_config_result(
            scope, &path, &backup, dry_run, uninstall,
        ));
    }
    let mut root: Value = match existing.as_deref() {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text)
            .with_context(|| format!("parse existing Cursor MCP config {}", path.display()))?,
        _ => json!({}),
    };
    let root_obj = root.as_object_mut().ok_or_else(|| {
        anyhow!(
            "Cursor MCP config must be a JSON object: {}",
            path.display()
        )
    })?;
    let servers = root_obj.entry("mcpServers").or_insert_with(|| json!({}));
    let servers_obj = servers.as_object_mut().ok_or_else(|| {
        anyhow!(
            "Cursor mcpServers must be a JSON object: {}",
            path.display()
        )
    })?;
    let desired_tfy = cursor_tfy_server_config(session);
    if uninstall {
        if let Some(existing_tfy) = servers_obj.get("tfy") {
            if !cursor_tfy_entry_is_managed(existing_tfy) {
                bail!(
                    "Cursor mcpServers.tfy is not TFY-owned; refusing to remove user-managed config in {}",
                    path.display()
                );
            }
        }
        servers_obj.remove("tfy");
        if servers_obj.is_empty() {
            root_obj.remove("mcpServers");
        }
    } else {
        if let Some(existing_tfy) = servers_obj.get("tfy") {
            if !cursor_tfy_entry_is_managed(existing_tfy) {
                bail!(
                    "Cursor mcpServers.tfy already exists and is not TFY-owned; refusing to overwrite user-managed config in {}",
                    path.display()
                );
            }
        }
        servers_obj.insert("tfy".into(), desired_tfy);
    }
    if !dry_run {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if uninstall && root.as_object().is_some_and(|object| object.is_empty()) {
            if path.exists() {
                fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
            }
            return Ok(cursor_config_result(
                scope, &path, &backup, dry_run, uninstall,
            ));
        }
        if existing.is_some() && !backup.exists() && !uninstall {
            fs::copy(&path, &backup)
                .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        }
        let rendered = format!("{}\n", serde_json::to_string_pretty(&root)?);
        fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(cursor_config_result(
        scope, &path, &backup, dry_run, uninstall,
    ))
}

fn cursor_tfy_server_config(session: &str) -> Value {
    json!({
        "command": "tfy",
        "args": [
            "mcp", "serve",
            "--session", session,
            "--ledger", ".tfy/mcp/ledger.jsonl",
            "--raw-dir", ".tfy/raw"
        ],
        "tfy_managed": true
    })
}

fn cursor_tfy_entry_is_managed(entry: &Value) -> bool {
    entry
        .get("tfy_managed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn cursor_config_result(
    scope: HostConfigScope,
    path: &Path,
    backup: &Path,
    dry_run: bool,
    uninstall: bool,
) -> Value {
    json!({
        "scope": scope.label(),
        "config_path": path.display().to_string(),
        "backup_path": backup.display().to_string(),
        "dry_run": dry_run,
        "applied": !dry_run,
        "action": if uninstall { "uninstall" } else { "install" },
    })
}

fn cursor_backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mcp.json");
    path.with_file_name(format!("{file_name}.tfy-backup"))
}

fn host_doctor_report(host: &str) -> Result<serde_json::Value> {
    let host = host_integration(host)?;
    Ok(json!({
        "host": host.id,
        "display": host.display,
        "status": host.status,
        "message": if host.status == "planned_discovery" {
            host.setup
        } else {
            "config snippet available; local MCP doctor checks prove TFY server health, not real host invocation"
        },
        "transport": host.transport,
        "config": host.config,
        "official_source": host.official_source,
        "evidence_gate": host.evidence_gate,
        "setup_success_is_not_savings_success": true,
        "launch_claim": host.launch_claim,
    }))
}

fn host_smoke_report(host: &str) -> Result<serde_json::Value> {
    let host = host_integration(host)?;
    Ok(json!({
        "host": host.id,
        "display": host.display,
        "status": if host.status == "planned_discovery" { "unsupported" } else { "checklist" },
        "claim_tier": if host.status == "planned_discovery" { "planned_discovery" } else { "configurable" },
        "message": if host.status == "planned_discovery" {
            host.setup
        } else {
            "host smoke is checklist/evidence collection until the named host actually invokes TFY; run `tfy smoke --mcp --json` for local MCP proof"
        },
        "artifact_contract": {
            "required_fields": [
                "host",
                "setup_verified",
                "real_invocation_verified",
                "setup_artifact",
                "invocation_artifact",
                "route_type",
                "config_scope",
                "config_path",
                "ledger_artifact",
                "raw_artifact",
                "redacted_public_bytes",
                "model_visible_bytes",
                "savings_result",
                "timestamp",
                "smoke_id"
            ],
            "launch_rule": "named hosts promote only when artifacts exist and bytes prove no-negative plus positive savings for that host route"
        },
        "local_mcp_command": "tfy smoke --mcp --json",
        "required_host_evidence": host.evidence_gate,
        "setup_success_is_not_savings_success": true,
    }))
}

fn selected_targets(project: bool, global: bool) -> Vec<ScopeTarget> {
    match (project, global) {
        (_, true) => vec![ScopeTarget::Global],
        (true, false) | (false, false) => vec![ScopeTarget::Project],
    }
}

fn install_codex(target: ScopeTarget, session: &str, dry_run: bool) -> Result<InitStatus> {
    let path = target.path();
    let block = codex_instruction_block(session);
    let existing = read_existing_text(&path)?;
    let next = upsert_marker_block(&existing, &block)?;
    if !dry_run {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&path, next)?;
    }
    let current = if dry_run {
        existing.clone()
    } else {
        read_existing_text(&path)?
    };
    Ok(InitStatus {
        action: "install".into(),
        target: target.label().into(),
        dry_run,
        applied: !dry_run,
        path: path.display().to_string(),
        installed: marker_present(&current),
        tiers: vec!["mcp_host_routed", "instruction_guidance"],
        warnings: vec![
            "MCP setup still requires the host to route tools through TFY; no private_hook is claimed.".into(),
            "P0 prints the Codex MCP command and does not directly mutate ~/.codex/config.toml.".into(),
        ],
    })
}

fn uninstall_codex(target: ScopeTarget, dry_run: bool) -> Result<InitStatus> {
    let path = target.path();
    let existing = read_existing_text(&path)?;
    let next = remove_marker_blocks(&existing)?;
    if !dry_run && existing != next {
        fs::write(&path, next)?;
    }
    let current = if dry_run {
        existing.clone()
    } else {
        read_existing_text(&path)?
    };
    Ok(InitStatus {
        action: "uninstall".into(),
        target: target.label().into(),
        dry_run,
        applied: !dry_run,
        path: path.display().to_string(),
        installed: marker_present(&current),
        tiers: vec!["instruction_guidance"],
        warnings: vec![
            "Only TFY-owned marker blocks are removed; non-TFY content is preserved.".into(),
        ],
    })
}

fn init_status(target: ScopeTarget) -> InitStatus {
    let path = target.path();
    let (existing, warning) = match read_existing_text(&path) {
        Ok(text) => (text, None),
        Err(err) => (
            String::new(),
            Some(format!("could not read {}: {err}", path.display())),
        ),
    };
    InitStatus {
        action: "show".into(),
        target: target.label().into(),
        dry_run: true,
        applied: false,
        path: path.display().to_string(),
        installed: marker_present(&existing),
        tiers: vec!["mcp_host_routed", "instruction_guidance"],
        warnings: warning
            .into_iter()
            .chain(if marker_present(&existing) {
                Vec::new()
            } else {
                vec!["TFY Codex marker block not found for this scope.".into()]
            })
            .collect(),
    }
}

fn print_init_results(results: &[InitStatus], session: &str) {
    for result in results {
        println!(
            "TFY init: action={} target={} dry_run={} applied={} path={}",
            result.action, result.target, result.dry_run, result.applied, result.path
        );
        println!("tiers={}", result.tiers.join(","));
        println!("marker_present={}", result.installed);
        if result.dry_run && result.action == "install" {
            println!("would_write marker block {TFY_CODEX_START} ... {TFY_CODEX_END}");
            println!("codex mcp add tfy -- tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw");
        }
        for warning in &result.warnings {
            println!("warning: {warning}");
        }
    }
}

fn codex_instruction_block(session: &str) -> String {
    format!(
        r#"{TFY_CODEX_START}
## TFY Codex integration

Integration tiers:
- mcp_host_routed: configure Codex MCP to run `tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw`.
- instruction_guidance: prefer TFY MCP tools for high-token command/context/output boundaries.
- private_hook: not claimed.

Recommended setup command:

```sh
codex mcp add tfy -- tfy mcp serve --session {session} --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
```

When available, prefer:
1. `tfy_tool_run` for noisy commands.
2. `tfy_scope_list` then exact-id `tfy_context_get` before reading whole files.
3. `tfy_output_validate` for preview and `tfy_output_apply` only with ApplyProof for writes.
4. `tfy_state_project` and `tfy_adapter_report` for compact session evidence.

Safety boundary: this is MCP host routing plus instructions, not private Codex hook interception, provider prompt interception, or universal shell interception.
{TFY_CODEX_END}
"#
    )
}

fn upsert_marker_block(existing: &str, block: &str) -> Result<String> {
    let without = remove_marker_blocks(existing)?;
    let mut out = without;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(block.trim_end());
    out.push('\n');
    Ok(out)
}

fn remove_marker_blocks(existing: &str) -> Result<String> {
    let mut out = String::new();
    let mut cursor = 0;
    while let Some(rel_start) = existing[cursor..].find(TFY_CODEX_START) {
        let start = cursor + rel_start;
        let preserved = &existing[cursor..start];
        if preserved.contains(TFY_CODEX_END) {
            bail!("malformed TFY Codex marker block: end marker without start marker");
        }
        out.push_str(preserved);
        let search_from = start + TFY_CODEX_START.len();
        let Some(end_rel) = existing[search_from..].find(TFY_CODEX_END) else {
            bail!("malformed TFY Codex marker block: missing end marker");
        };
        cursor = search_from + end_rel + TFY_CODEX_END.len();
    }
    if existing[cursor..].contains(TFY_CODEX_END) {
        bail!("malformed TFY Codex marker block: end marker without start marker");
    }
    out.push_str(&existing[cursor..]);
    Ok(out)
}

fn read_existing_text(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn marker_present(text: &str) -> bool {
    text.contains(TFY_CODEX_START) && text.contains(TFY_CODEX_END)
}

fn build_product_status_report() -> ProductStatusReport {
    ProductStatusReport {
        status: "active".into(),
        minimum_v1_host_matrix: minimum_v1_host_matrix(),
        surfaces: vec![
            SurfaceStatus { name: "command_output".into(), status: "active".into(), message: "AI-origin commands can route through tfy agent/adapter/MCP; normal human terminal commands are not intercepted.".into() },
            SurfaceStatus { name: "context_compacting".into(), status: "active".into(), message: "AI can list scopes and request exact compact function/file scopes instead of whole files.".into() },
            SurfaceStatus { name: "compact_code_restore".into(), status: "active".into(), message: "Compact one-line/short-symbol transport is restored to readable canonical code for files and user display.".into() },
            SurfaceStatus { name: "workspace_apply".into(), status: "active".into(), message: "Multi-file add/modify/delete/rename/move apply is proof-gated and rollback-journaled.".into() },
            SurfaceStatus { name: "fuzzy_refactor_apply".into(), status: "active".into(), message: "Fuzzy edits require unique anchors, confidence threshold, restored preview hashes, and conflict checks.".into() },
            SurfaceStatus { name: "codex_host_routing".into(), status: "config_snippet_available".into(), message: "TFY can configure supported Codex routing surfaces; launch support still requires host-bound smoke/ledger/raw evidence.".into() },
            SurfaceStatus { name: "ordinary_human_terminal".into(), status: "not_supported".into(), message: "TFY intentionally does not intercept regular terminal use.".into() },
            SurfaceStatus { name: "provider_api_gateway".into(), status: "not_supported".into(), message: "OpenAI/provider request proxying is outside TFY scope.".into() },
            SurfaceStatus { name: "editor_integration".into(), status: "not_supported".into(), message: "Editor auto-connection is outside TFY scope.".into() },
            SurfaceStatus { name: "private_codex_hook".into(), status: "not_supported".into(), message: "No private or hidden Codex prompt interception is claimed.".into() },
        ],
        claim_evidence_ladder: claim_evidence_ladder(),
        launch_claim_gate: "A host is launch-supported only after config snippet, config write/apply proof, host launch, verified host MCP or official hook invocation, route evidence, raw recovery, no-negative-savings, and positive-savings checks pass.".into(),
        not_supported: not_supported_surfaces(),
        truthfulness_boundary: "automatic configuration is limited to supported AI-host routes; no provider proxy, editor hook, private Codex hook, or universal shell interception".into(),
    }
}

fn claim_evidence_ladder() -> Vec<String> {
    [
        "config_snippet_available",
        "config_written",
        "host_launched",
        "verified_host_mcp_invocation",
        "verified_host_hook",
        "route_evidence_recorded",
        "savings_verified",
        "launch_supported",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn unsupported_ingress() -> Vec<String> {
    vec![
        "provider_api_prompt_proxy".into(),
        "private_codex_hook_interception".into(),
        "universal_terminal_interception".into(),
        "editor_internal_auto_hook_without_official_api_and_e2e".into(),
    ]
}

fn host_readiness_from_integration(host: &HostIntegration) -> HostReadiness {
    let claim_tier = if host.status == "planned_discovery" {
        "planned_discovery"
    } else {
        "configurable"
    };
    HostReadiness {
        host: host.id.into(),
        status: host.status.into(),
        required_for_v1: host.required_for_v1,
        claim_tier: claim_tier.into(),
        evidence_tiers: initial_host_evidence_tiers(host.status),
        next_evidence_tier: next_evidence_tier(&initial_host_evidence_tiers(host.status)).into(),
        supported_ingress: if host.status == "planned_discovery" {
            Vec::new()
        } else {
            vec!["mcp_stdio".into()]
        },
        equivalence_ingress: if host.status == "planned_discovery" {
            Vec::new()
        } else {
            vec!["official_host_hook_test_shim_only".into()]
        },
        unsupported_ingress: unsupported_ingress(),
        config_strategy: host.config_strategy.into(),
        apply_strategy: host.apply_strategy.into(),
        smoke_strategy: host.smoke_strategy.into(),
        host_evidence_strategy: host.host_evidence_strategy.into(),
        setup: host.setup.into(),
        normal_workflow: host.normal_workflow.into(),
        evidence_gate: host
            .evidence_gate
            .iter()
            .map(|item| (*item).into())
            .collect(),
        launch_claim: host.launch_claim.into(),
    }
}

fn initial_host_evidence_tiers(status: &str) -> Vec<String> {
    match status {
        "config_snippet_available" => vec!["config_snippet_available".into()],
        "planned_discovery" => vec!["planned_discovery".into()],
        "unsupported" => vec!["unsupported".into()],
        _ => vec![status.into()],
    }
}

fn next_evidence_tier(observed: &[String]) -> &'static str {
    if observed
        .iter()
        .any(|observed| observed == "planned_discovery")
    {
        return "official_route_discovery";
    }
    let has = |tier: &str| observed.iter().any(|observed| observed == tier);
    if has("launch_supported") {
        "complete"
    } else if has("savings_verified") {
        "launch_supported"
    } else if has("route_evidence_recorded") {
        "savings_verified"
    } else if has("verified_host_mcp_invocation") || has("verified_host_hook") {
        "route_evidence_recorded"
    } else if has("host_launched") {
        "verified_host_mcp_invocation_or_verified_host_hook"
    } else if has("config_written") {
        "host_launched"
    } else {
        "config_written"
    }
}

fn minimum_v1_host_matrix() -> Vec<HostReadiness> {
    let mut hosts = vec![
        HostReadiness {
            host: "mcp_stdio".into(),
            status: "not_configured".into(),
            required_for_v1: true,
            claim_tier: "not_configured".into(),
            evidence_tiers: Vec::new(),
            next_evidence_tier: "config_written".into(),
            supported_ingress: vec!["mcp_stdio".into()],
            equivalence_ingress: Vec::new(),
            unsupported_ingress: unsupported_ingress(),
            config_strategy: "host MCP stdio server entry pointing to `tfy mcp serve`".into(),
            apply_strategy: "host-specific apply; generic route requires explicit host config".into(),
            smoke_strategy: "local MCP initialize/tools-list plus host invocation artifact".into(),
            host_evidence_strategy: "host-bound MCP ledger/raw evidence and setup/invocation artifacts".into(),
            setup: "tfy mcp serve configured by the host".into(),
            normal_workflow: "host agent calls normal MCP tools/resources; user does not manually compact, paste refs, or edit raw ledgers on the happy path".into(),
            evidence_gate: vec![
                "initialize/tools-list".into(),
                "tool/context/output/state ledger events".into(),
                "raw_ref recovery".into(),
            ],
            launch_claim: "candidate launch route after MCP smoke and host-routed ledger evidence pass".into(),
        },
        HostReadiness {
            host: "tfy_agent_adapter".into(),
            status: "not_configured".into(),
            required_for_v1: true,
            claim_tier: "not_configured".into(),
            evidence_tiers: Vec::new(),
            next_evidence_tier: "config_written".into(),
            supported_ingress: vec!["agent_wrapper".into(), "generic_shell_adapter".into()],
            equivalence_ingress: Vec::new(),
            unsupported_ingress: unsupported_ingress(),
            config_strategy: "agent command executor uses `tfy agent` or `tfy adapter run`".into(),
            apply_strategy: "manual/host-owned executor wiring until a host-specific writer exists".into(),
            smoke_strategy: "adapter run smoke with ToolCommandCompleted ledger event".into(),
            host_evidence_strategy: "adapter ledger/raw evidence tied to configured wrapper invocation".into(),
            setup: "tfy agent or tfy adapter run wraps AI-origin command execution".into(),
            normal_workflow: "agent still requests ordinary commands; configured TFY wrapper preserves exit/status semantics while compacting model-visible feedback".into(),
            evidence_gate: vec![
                "ToolCommandCompleted ledger event".into(),
                "raw_ref recovery".into(),
                "no-negative-savings rendering".into(),
            ],
            launch_claim: "candidate launch route after configured runtime produces ledger/raw/no-negative-savings evidence".into(),
        },
        HostReadiness {
            host: "generic_shell".into(),
            status: "not_configured".into(),
            required_for_v1: true,
            claim_tier: "not_configured".into(),
            evidence_tiers: Vec::new(),
            next_evidence_tier: "config_written".into(),
            supported_ingress: vec!["generic_shell_adapter".into()],
            equivalence_ingress: Vec::new(),
            unsupported_ingress: unsupported_ingress(),
            config_strategy: "generic-shell adapter installed for an AI command executor only".into(),
            apply_strategy: "`tfy adapter install --target generic-shell` where the caller opts in".into(),
            smoke_strategy: "wrapper invocation and adapter report smoke".into(),
            host_evidence_strategy: "wrapper-origin ledger/raw evidence; ordinary terminals remain out of scope".into(),
            setup: "tfy adapter install --target generic-shell".into(),
            normal_workflow: "only the configured agent command executor is wrapped; ordinary human terminals are never globally intercepted".into(),
            evidence_gate: vec![
                "configured wrapper invocation".into(),
                "adapter ledger report".into(),
                "raw_ref recovery".into(),
            ],
            launch_claim: "candidate launch route after wrapper invocation and adapter ledger evidence pass; ordinary human terminals are untouched"
                .into(),
        },
        HostReadiness {
            host: "codex".into(),
            status: "config_snippet_available".into(),
            required_for_v1: false,
            claim_tier: "configurable".into(),
            evidence_tiers: vec!["config_snippet_available".into()],
            next_evidence_tier: "config_written".into(),
            supported_ingress: vec!["mcp_stdio".into()],
            equivalence_ingress: vec!["official_host_hook_test_shim_only".into()],
            unsupported_ingress: unsupported_ingress(),
            config_strategy: "Codex MCP command/TOML setup plus project AGENTS.md guidance".into(),
            apply_strategy: "project AGENTS.md apply via `tfy init`; Codex TOML remains dry-run/manual".into(),
            smoke_strategy: "local MCP smoke plus real Codex invocation artifact".into(),
            host_evidence_strategy: "Codex-bound MCP ledger/raw evidence with config path and smoke id".into(),
            setup: "Codex MCP config plus TFY AGENTS.md guidance".into(),
            normal_workflow: "Codex remains normal only after official/configurable MCP or hook routing proves real host invocation; checklist-only guidance is not launch support".into(),
            evidence_gate: vec![
                "Codex host actually invokes TFY MCP".into(),
                "ledger events from Codex session".into(),
                "raw recovery and no-negative-savings".into(),
            ],
            launch_claim: "not launch-supported until real Codex invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists".into(),
        },
    ];
    for host in host_registry()
        .into_iter()
        .filter(|host| host.id != "codex")
    {
        hosts.push(host_readiness_from_integration(&host));
    }
    hosts
}

fn not_supported_surfaces() -> Vec<String> {
    vec![
        "provider_api_gateway".into(),
        "provider_api_prompt_proxy".into(),
        "editor_integration".into(),
        "private_codex_hook".into(),
        "private_codex_hook_interception".into(),
        "ordinary_human_terminal_interception".into(),
        "universal_terminal_interception".into(),
        "unconfigured_hosts".into(),
    ]
}

fn build_host_evidence_summary(
    ledgers: &[PathBuf],
    session: Option<&str>,
    host_evidence_files: &[PathBuf],
) -> HostEvidenceSummary {
    let mut summary = HostEvidenceSummary {
        no_negative_savings: true,
        generic_shell_route: RouteEvidence {
            no_negative_savings: true,
            ..Default::default()
        },
        tfy_agent_adapter_route: RouteEvidence {
            no_negative_savings: true,
            ..Default::default()
        },
        mcp_stdio_route: RouteEvidence {
            no_negative_savings: true,
            ..Default::default()
        },
        ..Default::default()
    };
    for ledger in ledgers {
        let Ok(events) = load_events(ledger) else {
            continue;
        };
        let ledger_hint = ledger.to_string_lossy();
        for event in events {
            if let Some(session_filter) = session {
                if event.session_id != session_filter {
                    continue;
                }
            }
            match &event.payload {
                GatewayEvent::ToolCommandCompleted {
                    raw_ref,
                    raw_bytes,
                    model_bytes,
                    raw_chars,
                    summary_chars,
                    model_chars,
                    negative_savings_avoided,
                    rendering_kind,
                    ..
                } => {
                    summary.commands += 1;
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
                    if model_size > raw_size && !negative_savings_avoided {
                        summary.no_negative_savings = false;
                    }
                    if raw_size > model_size {
                        summary.positive_savings = true;
                    }
                    if event.adapter_kind == AdapterKind::Mcp {
                        summary.mcp_tool = true;
                        update_route_tool_evidence(
                            &mut summary.mcp_stdio_route,
                            raw_ref,
                            event.provenance.raw_refs.len(),
                            raw_size,
                            model_size,
                            *negative_savings_avoided,
                        );
                    } else if event.adapter_kind == AdapterKind::Cli
                        && matches!(event.origin.invocation, OriginInvocation::Wrapper)
                    {
                        summary.tfy_agent_adapter = true;
                        update_route_tool_evidence(
                            &mut summary.tfy_agent_adapter_route,
                            raw_ref,
                            event.provenance.raw_refs.len(),
                            raw_size,
                            model_size,
                            *negative_savings_avoided,
                        );
                    } else if matches!(event.adapter_kind, AdapterKind::Shell)
                        && (ledger_hint.contains("adapter")
                            || matches!(event.origin.invocation, OriginInvocation::Wrapper))
                    {
                        summary.generic_shell_wrapper = true;
                        update_route_tool_evidence(
                            &mut summary.generic_shell_route,
                            raw_ref,
                            event.provenance.raw_refs.len(),
                            raw_size,
                            model_size,
                            *negative_savings_avoided,
                        );
                    }
                    if event.origin.host == tfy_runtime::OriginHost::Codex
                        && event.origin.intercepted
                        && !matches!(event.origin.invocation, OriginInvocation::PrivateHook)
                    {
                        summary.codex_real_invocation = true;
                    }
                    if !rendering_kind.is_empty() {
                        summary
                            .evidence_notes
                            .push(format!("tool rendering_kind={rendering_kind}"));
                    }
                }
                GatewayEvent::ContextSelected { .. } => {
                    if event.adapter_kind == AdapterKind::Mcp {
                        summary.mcp_context = true;
                        summary.mcp_stdio_route.context = true;
                    }
                }
                GatewayEvent::OutputValidated { .. } => {
                    if event.adapter_kind == AdapterKind::Mcp {
                        summary.mcp_output = true;
                        summary.mcp_stdio_route.output = true;
                    }
                }
                GatewayEvent::StateProjected { .. } => {
                    if event.adapter_kind == AdapterKind::Mcp {
                        summary.mcp_state = true;
                        summary.mcp_stdio_route.state = true;
                    }
                }
                GatewayEvent::Fallback { .. }
                | GatewayEvent::Validation { .. }
                | GatewayEvent::Error { .. } => {}
            }
        }
    }
    apply_host_setup_evidence(&mut summary, host_evidence_files);
    summary.raw_refs = summary.generic_shell_route.raw_refs
        + summary.tfy_agent_adapter_route.raw_refs
        + summary.mcp_stdio_route.raw_refs
        + summary
            .named_hosts
            .values()
            .map(|route| route.raw_refs)
            .sum::<usize>();
    if summary.commands == 0 {
        summary.no_negative_savings = false;
    }
    summary
}

fn update_route_tool_evidence(
    route: &mut RouteEvidence,
    raw_ref: &str,
    provenance_raw_refs: usize,
    raw_size: usize,
    model_size: usize,
    negative_savings_avoided: bool,
) {
    route.commands += 1;
    route.tool = true;
    route.raw_refs += provenance_raw_refs;
    if !raw_ref.is_empty() && provenance_raw_refs == 0 {
        route.raw_refs += 1;
    }
    if model_size > raw_size && !negative_savings_avoided {
        route.no_negative_savings = false;
    }
    if raw_size > model_size {
        route.positive_savings = true;
    }
}

fn apply_host_setup_evidence(summary: &mut HostEvidenceSummary, files: &[PathBuf]) {
    for path in files {
        let Ok(text) = fs::read_to_string(path) else {
            summary
                .evidence_notes
                .push(format!("host evidence file unreadable: {}", path.display()));
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<HostEvidenceFile>(&text) else {
            summary.evidence_notes.push(format!(
                "host evidence file invalid JSON: {}",
                path.display()
            ));
            continue;
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        for host in parsed.hosts {
            let route = match host.host.as_str() {
                "generic_shell" => Some(&mut summary.generic_shell_route),
                "tfy_agent_adapter" => Some(&mut summary.tfy_agent_adapter_route),
                "mcp_stdio" => Some(&mut summary.mcp_stdio_route),
                _ => None,
            };
            if let Some(route) = route {
                let setup_artifact_verified = host
                    .setup_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let invocation_artifact_verified = host
                    .invocation_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let overhead_measured = host.overhead_ms.is_some() && host.baseline_ms.is_some();
                let overhead_passed = match (host.overhead_ms, host.baseline_ms) {
                    (Some(overhead), Some(baseline)) => {
                        overhead <= baseline.saturating_mul(2)
                            && overhead <= baseline.saturating_add(5_000)
                    }
                    _ => false,
                } || host
                    .overhead_exception
                    .as_deref()
                    .is_some_and(|exception| !exception.trim().is_empty());
                route.setup_artifact_verified |= setup_artifact_verified;
                route.invocation_artifact_verified |= invocation_artifact_verified;
                route.setup_verified |= host.setup_verified && setup_artifact_verified;
                route.real_invocation_verified |=
                    host.real_invocation_verified && invocation_artifact_verified;
                route.overhead_measured |= overhead_measured;
                route.overhead_passed |= overhead_passed;
                route.overhead_ms = route.overhead_ms.or(host.overhead_ms);
                route.baseline_ms = route.baseline_ms.or(host.baseline_ms);
                if route.overhead_exception.is_none() {
                    route.overhead_exception = host.overhead_exception;
                }
                summary.evidence_notes.push(format!(
                    "host_evidence {} setup_verified={} setup_artifact_verified={} real_invocation_verified={} invocation_artifact_verified={} overhead_measured={} overhead_passed={}",
                    host.host,
                    host.setup_verified,
                    setup_artifact_verified,
                    host.real_invocation_verified,
                    invocation_artifact_verified,
                    overhead_measured,
                    overhead_passed
                ));
            } else if host_accepts_launch_evidence(&host.host) {
                let setup_artifact_verified = host
                    .setup_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let invocation_artifact_verified = host
                    .invocation_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let overhead_measured = host.overhead_ms.is_some() && host.baseline_ms.is_some();
                let overhead_passed = match (host.overhead_ms, host.baseline_ms) {
                    (Some(overhead), Some(baseline)) => {
                        overhead <= baseline.saturating_mul(2)
                            && overhead <= baseline.saturating_add(5_000)
                    }
                    _ => false,
                } || host
                    .overhead_exception
                    .as_deref()
                    .is_some_and(|exception| !exception.trim().is_empty());
                let route = summary
                    .named_hosts
                    .entry(host.host.clone())
                    .or_insert_with(|| RouteEvidence {
                        no_negative_savings: true,
                        ..Default::default()
                    });
                route.setup_artifact_verified |= setup_artifact_verified;
                route.invocation_artifact_verified |= invocation_artifact_verified;
                route.setup_verified |= host.setup_verified && setup_artifact_verified;
                route.real_invocation_verified |=
                    host.real_invocation_verified && invocation_artifact_verified;
                route.overhead_measured |= overhead_measured;
                route.overhead_passed |= overhead_passed;
                route.overhead_ms = route.overhead_ms.or(host.overhead_ms);
                route.baseline_ms = route.baseline_ms.or(host.baseline_ms);
                if route.overhead_exception.is_none() {
                    route.overhead_exception = host.overhead_exception;
                }
                let host_id_matches = host
                    .host_id
                    .as_deref()
                    .is_none_or(|host_id| host_id == host.host);
                let route_type_valid = non_empty_opt(&host.route_type);
                let config_scope_valid = non_empty_opt(&host.config_scope);
                let smoke_id_valid = non_empty_opt(&host.smoke_id);
                let timestamp_valid = non_empty_opt(&host.timestamp);
                let config_path_verified = host
                    .config_path
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let ledger_artifact_verified = host
                    .ledger_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let raw_artifact_verified = host
                    .raw_artifact
                    .as_ref()
                    .is_some_and(|artifact| host_artifact_exists(base, artifact));
                let (no_negative, positive) =
                    match (host.redacted_public_bytes, host.model_visible_bytes) {
                        (Some(raw), Some(model)) => (model <= raw, raw > model),
                        _ => {
                            // Named hosts cannot launch from self-reported savings labels alone.
                            // Exact raw/model byte fields are required to prove no-negative and
                            // positive savings for the host-bound route.
                            (true, false)
                        }
                    };
                let hook_route = matches!(
                    host.route_type.as_deref(),
                    Some("official_host_hook" | "host_hook" | "hook")
                );
                let hook_authorized = !hook_route
                    || (host.official_docs_backed == Some(true)
                        && host.kill_switch_available == Some(true)
                        && host.uninstall_available == Some(true)
                        && host_official_hook_launch_supported(&host.host));
                let host_bound = host_id_matches
                    && route_type_valid
                    && config_scope_valid
                    && config_path_verified
                    && ledger_artifact_verified
                    && raw_artifact_verified
                    && smoke_id_valid
                    && timestamp_valid
                    && hook_authorized
                    && no_negative
                    && positive;
                route.host_bound_evidence |= host_bound;
                route.official_docs_backed |= host.official_docs_backed == Some(true);
                route.kill_switch_available |= host.kill_switch_available == Some(true);
                route.uninstall_available |= host.uninstall_available == Some(true);
                route.no_negative_savings &= no_negative;
                route.positive_savings |= positive;
                if host_bound {
                    route.commands += 1;
                    route.raw_refs += 1;
                    route.tool = true;
                    if route.route_type.is_none() {
                        route.route_type = host.route_type.clone();
                    }
                    if route.config_scope.is_none() {
                        route.config_scope = host.config_scope.clone();
                    }
                    if route.config_path.is_none() {
                        route.config_path = host
                            .config_path
                            .as_ref()
                            .map(|path| path.display().to_string());
                    }
                    if route.smoke_id.is_none() {
                        route.smoke_id = host.smoke_id.clone();
                    }
                    if route.host_version.is_none() {
                        route.host_version = host.host_version.clone();
                    }
                    summary.commands += 1;
                    summary.raw_refs += 1;
                    summary.no_negative_savings &= no_negative;
                    summary.positive_savings |= positive;
                }
                summary.evidence_notes.push(format!(
                    "named_host_evidence {} setup_verified={} setup_artifact_verified={} real_invocation_verified={} invocation_artifact_verified={} overhead_measured={} overhead_passed={} host_bound_evidence={} config_path_verified={} ledger_artifact_verified={} raw_artifact_verified={} hook_route={} hook_authorized={}",
                    host.host,
                    host.setup_verified,
                    setup_artifact_verified,
                    host.real_invocation_verified,
                    invocation_artifact_verified,
                    overhead_measured,
                    overhead_passed,
                    host_bound,
                    config_path_verified,
                    ledger_artifact_verified,
                    raw_artifact_verified,
                    hook_route,
                    hook_authorized
                ));
            }
        }
    }
}

fn non_empty_opt(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn host_accepts_launch_evidence(host: &str) -> bool {
    host_integration(host).is_ok_and(|integration| integration.status != "planned_discovery")
}

fn host_official_hook_launch_supported(_host: &str) -> bool {
    // No named production host currently has an implemented official-hook writer with
    // docs, uninstall, kill-switch, and e2e evidence. The hook surface is test-shim
    // only until a host is explicitly enabled here with regression coverage.
    false
}

fn host_artifact_exists(base: &Path, artifact: &Path) -> bool {
    let path = if artifact.is_absolute() {
        artifact.to_path_buf()
    } else {
        base.join(artifact)
    };
    path.is_file()
}

fn apply_launch_evidence(
    mut hosts: Vec<HostReadiness>,
    gain: &GainReport,
    evidence: &HostEvidenceSummary,
) -> Vec<HostReadiness> {
    let has_repeat_evidence = gain
        .rendering_counts
        .get("repeat_elided")
        .copied()
        .unwrap_or(0)
        > 0;
    for host in &mut hosts {
        let (local_verified, launch_supported, route) = match host.host.as_str() {
            "generic_shell" => {
                let route = &evidence.generic_shell_route;
                let local = evidence.generic_shell_wrapper
                    && route.commands > 0
                    && route.raw_refs > 0
                    && route.no_negative_savings
                    && route.positive_savings;
                (local, local && route_host_ready(route), Some(route))
            }
            "tfy_agent_adapter" => {
                let route = &evidence.tfy_agent_adapter_route;
                let local = evidence.tfy_agent_adapter
                    && route.commands > 0
                    && route.raw_refs > 0
                    && route.no_negative_savings
                    && route.positive_savings;
                (local, local && route_host_ready(route), Some(route))
            }
            "mcp_stdio" => {
                let route = &evidence.mcp_stdio_route;
                let local = evidence.mcp_tool
                    && evidence.mcp_context
                    && evidence.mcp_output
                    && evidence.mcp_state
                    && route.raw_refs > 0
                    && route.no_negative_savings
                    && route.positive_savings;
                (local, local && route_host_ready(route), Some(route))
            }
            "openclaw" => (false, false, None),
            "codex" => {
                let named = evidence.named_hosts.get("codex");
                let local = named
                    .is_some_and(|route| route.setup_verified || route.real_invocation_verified);
                let launch = named.is_some_and(named_host_ready);
                (local || launch, launch, named)
            }
            other => {
                let named = evidence.named_hosts.get(other);
                let local = named
                    .is_some_and(|route| route.setup_verified || route.real_invocation_verified);
                let launch = named.is_some_and(named_host_ready);
                (local || launch, launch, named)
            }
        };
        if launch_supported {
            host.evidence_tiers = route
                .map(|route| host_observed_evidence_tiers(&host.host, route))
                .unwrap_or_else(claim_evidence_ladder);
            if !host
                .evidence_tiers
                .iter()
                .any(|tier| tier == "launch_supported")
            {
                host.evidence_tiers.push("launch_supported".into());
            }
            host.next_evidence_tier = next_evidence_tier(&host.evidence_tiers).into();
            host.status = "launch_supported".into();
            host.claim_tier = "launch_supported".into();
            host.launch_claim =
                "launch-supported for this report: setup, real host invocation, host-bound ledger evidence, raw recovery, no-negative-savings, and positive-savings gates passed"
                    .into();
        } else if local_verified {
            host.evidence_tiers = route
                .map(|route| host_observed_evidence_tiers(&host.host, route))
                .unwrap_or_else(|| initial_host_evidence_tiers(&host.status));
            host.next_evidence_tier = next_evidence_tier(&host.evidence_tiers).into();
            host.status = if route.is_some_and(|route| route.real_invocation_verified) {
                "verified_host_invocation"
            } else if route.is_some_and(|route| route.setup_verified) {
                "applied_unverified"
            } else {
                "verified_local_mcp"
            }
            .into();
            host.claim_tier = if route.is_some_and(|route| route.real_invocation_verified) {
                "smoke_routed"
            } else if route.is_some_and(|route| route.setup_verified) {
                "applied_unverified"
            } else {
                "verified_local_mcp"
            }
            .into();
            host.launch_claim =
                "local smoke verified, but not launch-supported until host setup, real invocation, and route-bound TFY ledger/raw/no-negative/positive-savings evidence are supplied"
                    .into();
        }
        if local_verified || launch_supported {
            if let Some(route) = route {
                host.evidence_gate.push(format!(
                    "observed route_commands={} route_raw_refs={} route_positive_savings={} route_no_negative_savings={}",
                    route.commands, route.raw_refs, route.positive_savings, route.no_negative_savings
                ));
                if route.overhead_measured {
                    host.evidence_gate.push(format!(
                        "observed route_overhead_ms={} route_baseline_ms={}",
                        route.overhead_ms.unwrap_or_default(),
                        route.baseline_ms.unwrap_or_default()
                    ));
                }
                if let Some(exception) = &route.overhead_exception {
                    host.evidence_gate
                        .push(format!("observed route_overhead_exception={exception}"));
                }
                if route.host_bound_evidence {
                    host.evidence_gate.push(format!(
                        "observed host_bound_evidence=true route_type={} config_scope={} config_path={} smoke_id={}",
                        route.route_type.as_deref().unwrap_or("unknown"),
                        route.config_scope.as_deref().unwrap_or("unknown"),
                        route.config_path.as_deref().unwrap_or("unknown"),
                        route.smoke_id.as_deref().unwrap_or("unknown")
                    ));
                }
            } else {
                host.evidence_gate.push(format!(
                    "observed route_raw_refs={} codex_real_invocation={}",
                    evidence.raw_refs, evidence.codex_real_invocation
                ));
            }
            if has_repeat_evidence {
                host.evidence_gate
                    .push("observed repeat_elided unchanged-output savings".into());
            }
        }
    }
    hosts
}

fn host_observed_evidence_tiers(host: &str, route: &RouteEvidence) -> Vec<String> {
    let mut tiers = Vec::new();
    if route.setup_artifact_verified {
        tiers.push("config_written".into());
    }
    if route.invocation_artifact_verified {
        tiers.push("host_launched".into());
    }
    if route.real_invocation_verified && route.tool {
        match route.route_type.as_deref() {
            Some("official_host_hook") | Some("host_hook") | Some("hook") => {
                tiers.push("verified_host_hook".into());
            }
            Some("mcp_stdio") | Some("mcp") | None if host == "mcp_stdio" => {
                tiers.push("verified_host_mcp_invocation".into());
            }
            Some("mcp_stdio") | Some("mcp") => {
                tiers.push("verified_host_mcp_invocation".into());
            }
            _ => tiers.push("verified_host_invocation".into()),
        }
    }
    if route.host_bound_evidence || route.commands > 0 || route.raw_refs > 0 {
        tiers.push("route_evidence_recorded".into());
    }
    if route.raw_refs > 0 && route.no_negative_savings && route.positive_savings {
        tiers.push("savings_verified".into());
    }
    if named_host_ready(route) {
        tiers.push("launch_supported".into());
    }
    tiers
}

fn named_host_ready(route: &RouteEvidence) -> bool {
    route_host_ready(route)
        && route.host_bound_evidence
        && route.commands > 0
        && route.raw_refs > 0
        && route.no_negative_savings
        && route.positive_savings
}

fn route_host_ready(route: &RouteEvidence) -> bool {
    let hook_route = matches!(
        route.route_type.as_deref(),
        Some("official_host_hook" | "host_hook" | "hook")
    );
    route.setup_verified
        && route.real_invocation_verified
        && route.setup_artifact_verified
        && route.invocation_artifact_verified
        && (!hook_route
            || (route.official_docs_backed
                && route.kill_switch_available
                && route.uninstall_available))
        && (route.overhead_measured || route.overhead_exception.is_some())
        && route.overhead_passed
}

fn build_launch_readiness_report(
    status: ProductStatusReport,
    gain: GainReport,
    host_evidence: HostEvidenceSummary,
) -> LaunchReadinessReport {
    let mut blockers = Vec::new();
    let host_matrix = apply_launch_evidence(status.minimum_v1_host_matrix, &gain, &host_evidence);
    for host in &host_matrix {
        if host.required_for_v1 && host.status != "launch_supported" {
            blockers.push(format!(
                "required v1 host {} is {}; launch-report cannot pass until setup + real invocation + ledger + raw recovery + no-negative-savings evidence pass",
                host.host, host.status
            ));
        }
    }
    if gain.saved_bytes < 0 {
        blockers.push("negative savings detected in gain ledgers".into());
    }
    if gain.commands == 0 {
        blockers.push("no command-output savings data found; run adapter/MCP tool workflows before launch claims".into());
    }
    if !host_evidence.no_negative_savings {
        blockers.push("negative default model-visible savings or missing no-negative-savings evidence detected".into());
    }
    if !host_evidence.positive_savings {
        blockers
            .push("no positive saved byte/token evidence for required command-heavy routes".into());
    }
    LaunchReadinessReport {
        status: if blockers.is_empty() {
            "pass"
        } else {
            "blocked"
        }
        .into(),
        host_matrix,
        claim_evidence_ladder: claim_evidence_ladder(),
        gain,
        host_evidence,
        release_thresholds: ReleaseThresholds::default(),
        measurement_method: MeasurementMethod::default(),
        privacy_raw_store: PrivacyRawStorePolicy::default(),
        overhead_policy: OverheadPolicy::default(),
        unsupported_claim_audit: UnsupportedClaimAudit {
            status: "pass".into(),
            audited_claims: not_supported_surfaces(),
            rule: "unsupported provider/API prompt proxy, editor-internal auto hook, private Codex hook interception, and universal terminal interception claims must remain not_supported/planned; MCP and official-host-hook routes may promote only through config_written, host_launched, verified host invocation, route evidence, and savings_verified gates".into(),
        },
        blockers,
        not_supported: status.not_supported,
        required_benchmark_scenarios: vec![
            "command-heavy debugging".into(),
            "context-heavy code edit".into(),
            "repeated test loop".into(),
            "Git/GitHub evidence".into(),
            "long-session state compaction".into(),
            "MCP Code I/O workflow".into(),
            "host setup failure recovery".into(),
        ],
    }
}

fn build_explain_report() -> ProductExplainReport {
    ProductExplainReport {
        ai_transport: "AI sees compact scope/function-level code such as `function f0(a,b){const c=a+b;return c;}` when that saves tokens.".into(),
        file_and_user_output: "Before code is written or shown to a human, TFY restores original/readable names, indentation, and line breaks into canonical file code.".into(),
        automatic_routing: "TFY can automatically configure supported AI-agent host routes where safe writers exist, then the host can use TFY through wrapper/adapter/MCP setup; launch support is granted only after host-bound evidence proves routing and savings.".into(),
        apply_model: "Writes are validated with plan hash, per-operation proof, preview hash, origin checks, safe paths, rollback journal, and fuzzy unique-anchor gates.".into(),
        what_tfy_changed: vec![
            "configured AI command/context/output boundaries route through TFY instead of sending raw noisy payloads directly to the model".into(),
            "ordinary human terminals are not globally intercepted".into(),
            "launch support is evidence-gated per host route".into(),
        ],
        data_stored_locally: vec![
            "raw command/context evidence under .tfy/raw or configured --raw-dir".into(),
            "gateway ledgers such as .tfy/mcp/ledger.jsonl and .tfy/adapter/ledger.jsonl".into(),
            "compact state projections derived from local ledgers".into(),
        ],
        raw_recovery: "Use raw_ref values with `tfy raw` or MCP `tfy_raw_get`/`tfy://raw/{raw_ref}` resources to recover byte-exact evidence.".into(),
        deletion_export: "Until first-class retention commands are added, delete/export local evidence by managing the .tfy raw and ledger paths listed by doctor/explain; TFY does not upload raw evidence.".into(),
        launch_pass_block_reason: "`tfy launch-report` passes only when required v1 hosts have setup + real invocation + host-bound ledger/raw recovery + no-negative/positive-savings evidence and all claim/privacy gates pass.".into(),
        out_of_scope: vec![
            "provider/API gateway proxy".into(),
            "editor auto-integration".into(),
            "private hidden Codex prompt hook".into(),
            "ordinary human terminal interception".into(),
        ],
    }
}

fn build_doctor_report(codex: bool) -> DoctorReport {
    let mut diagnostics = Vec::new();
    diagnostics.push(Diagnostic {
        name: "binary".into(),
        status: "pass".into(),
        message: format!("tfy {} is running", env!("CARGO_PKG_VERSION")),
    });
    diagnostics.extend(check_mcp_server());
    if ensure_writable_dir(Path::new(".tfy/mcp")).is_ok()
        && ensure_writable_dir(Path::new(".tfy/raw")).is_ok()
    {
        diagnostics.push(Diagnostic {
            name: "writable_dirs".into(),
            status: "pass".into(),
            message: ".tfy/mcp and .tfy/raw are writable".into(),
        });
    } else {
        diagnostics.push(Diagnostic {
            name: "writable_dirs".into(),
            status: "fail".into(),
            message: "could not create/write .tfy/mcp or .tfy/raw".into(),
        });
    }
    if codex {
        for target in [ScopeTarget::Project, ScopeTarget::Global] {
            let status = init_status(target);
            diagnostics.push(Diagnostic {
                name: format!("codex_{}_instructions", target.label()),
                status: if status.installed { "pass" } else { "warn" }.into(),
                message: if status.installed {
                    format!("TFY marker block found at {}", status.path)
                } else {
                    format!(
                        "TFY marker block not found at {}; run tfy init --codex --{} --apply",
                        status.path,
                        target.label()
                    )
                },
            });
        }
        diagnostics.push(Diagnostic {
            name: "codex_mcp_config".into(),
            status: "warn".into(),
            message: "P0 does not mutate ~/.codex/config.toml; verify Codex MCP config manually or with `codex mcp list`.".into(),
        });
        for name in [
            "private_codex_hook",
            "provider_api_gateway",
            "editor_integration",
            "ordinary_human_terminal_interception",
        ] {
            diagnostics.push(Diagnostic {
                name: name.into(),
                status: "pass".into(),
                message: "intentionally not claimed or intercepted by TFY".into(),
            });
        }
    }
    let status = if diagnostics.iter().any(|d| d.status == "fail") {
        "fail"
    } else if diagnostics.iter().any(|d| d.status == "warn") {
        "warn"
    } else {
        "pass"
    };
    DoctorReport {
        status: status.into(),
        diagnostics,
    }
}

fn run_adapter_smoke() -> Result<SmokeReport> {
    let root = std::env::temp_dir().join(format!(
        "tfy-adapter-smoke-{}-{}",
        std::process::id(),
        stable_id("adapter-smoke")
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let ledger = root.join("adapter-ledger.jsonl");
    let raw = root.join("raw");
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let output = Command::new(exe)
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "adapter",
            "run",
            "--session",
            "smoke",
            "--ledger",
            ledger
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 ledger path"))?,
            "--raw-dir",
            raw.to_str().ok_or_else(|| anyhow!("non-utf8 raw path"))?,
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo tfy-adapter-smoke-$i; done",
        ])
        .output()
        .context("run adapter smoke command")?;
    if !output.status.success() {
        bail!(
            "adapter smoke command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let events = load_events(&ledger).context("read adapter smoke ledger")?;
    let has_raw_ref = events
        .iter()
        .any(|event| !event.provenance.raw_refs.is_empty());
    if !has_raw_ref {
        bail!("adapter smoke did not record raw refs");
    }
    Ok(SmokeReport {
        status: "pass".into(),
        mode: "adapter".into(),
        sample_path: ledger.display().to_string(),
        scope_id: "tool-command".into(),
        preview_applied: false,
        apply_applied: false,
        ledger_events: events.len(),
        evidence: vec![
            "generic-shell local smoke produced ToolCommandCompleted ledger evidence".into(),
            "adapter local smoke preserved raw_ref recovery evidence".into(),
            format!("ledger={}", ledger.display()),
            format!("raw_dir={}", raw.display()),
            format!("ledger_events={}", events.len()),
        ],
    })
}

fn run_agent_smoke() -> Result<SmokeReport> {
    let root = std::env::temp_dir().join(format!(
        "tfy-agent-smoke-{}-{}",
        std::process::id(),
        stable_id("agent-smoke")
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let ledger = root.join("agent-ledger.jsonl");
    let raw = root.join("raw");
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let output = Command::new(exe)
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "agent",
            "run",
            "--session",
            "smoke",
            "--host",
            "generic",
            "--ledger",
            ledger
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 ledger path"))?,
            "--raw-dir",
            raw.to_str().ok_or_else(|| anyhow!("non-utf8 raw path"))?,
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo tfy-agent-smoke-$i; done",
        ])
        .output()
        .context("run agent smoke command")?;
    if !output.status.success() {
        bail!(
            "agent smoke command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let events = load_events(&ledger).context("read agent smoke ledger")?;
    let has_raw_ref = events
        .iter()
        .any(|event| !event.provenance.raw_refs.is_empty());
    if !has_raw_ref {
        bail!("agent smoke did not record raw refs");
    }
    Ok(SmokeReport {
        status: "pass".into(),
        mode: "agent".into(),
        sample_path: ledger.display().to_string(),
        scope_id: "tool-command".into(),
        preview_applied: false,
        apply_applied: false,
        ledger_events: events.len(),
        evidence: vec![
            "tfy-agent local smoke produced ToolCommandCompleted ledger evidence".into(),
            "agent local smoke preserved raw_ref recovery evidence".into(),
            format!("ledger={}", ledger.display()),
            format!("raw_dir={}", raw.display()),
            format!("ledger_events={}", events.len()),
        ],
    })
}

fn check_mcp_server() -> Vec<Diagnostic> {
    match start_mcp_child("doctor") {
        Ok(mut child) => {
            let init = child.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}));
            let tools = child.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
            match (init, tools) {
                (Ok(init), Ok(tools)) => {
                    let mut diagnostics = vec![Diagnostic {
                        name: "mcp_initialize".into(),
                        status: if init.get("result").is_some() {
                            "pass"
                        } else {
                            "fail"
                        }
                        .into(),
                        message: "tfy mcp serve answered initialize".into(),
                    }];
                    let names = tool_names(&tools);
                    let missing: Vec<_> = REQUIRED_MCP_TOOLS
                        .iter()
                        .filter(|name| !names.iter().any(|n| n == **name))
                        .copied()
                        .collect();
                    diagnostics.push(Diagnostic {
                        name: "mcp_tools".into(),
                        status: if missing.is_empty() { "pass" } else { "fail" }.into(),
                        message: if missing.is_empty() {
                            format!(
                                "required MCP tools present: {}",
                                REQUIRED_MCP_TOOLS.join(", ")
                            )
                        } else {
                            format!("missing MCP tools: {}", missing.join(", "))
                        },
                    });
                    diagnostics
                }
                (Err(e), _) | (_, Err(e)) => vec![Diagnostic {
                    name: "mcp_server".into(),
                    status: "fail".into(),
                    message: e.to_string(),
                }],
            }
        }
        Err(e) => vec![Diagnostic {
            name: "mcp_server".into(),
            status: "fail".into(),
            message: e.to_string(),
        }],
    }
}

fn run_mcp_smoke() -> Result<SmokeReport> {
    let root = std::env::temp_dir().join(format!(
        "tfy-smoke-{}-{}",
        std::process::id(),
        stable_id("mcp-smoke")
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let sample = root.join("sample.js");
    fs::write(
        &sample,
        "function add(left, right) {\n  const total = left + right;\n  return total;\n}\n",
    )?;
    let ledger = root.join("ledger.jsonl");
    let raw = root.join("raw");
    let mut child = start_mcp_child_with_paths("smoke", &ledger, &raw)?;
    child.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}))?;
    let tools = child.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))?;
    let names = tool_names(&tools);
    for required in REQUIRED_MCP_TOOLS {
        if !names.iter().any(|name| name == required) {
            bail!("missing tool {required}");
        }
    }
    let tool_run = child.request(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tfy_tool_run","arguments":{"session":"smoke","command":["sh","-c","for i in $(seq 1 80); do echo tfy-mcp-smoke-$i; done"]}}}))?;
    let tool_run_json = mcp_content_json(&tool_run)?;
    if tool_run_json["payload"]["raw_ref"]
        .as_str()
        .unwrap_or_default()
        .is_empty()
    {
        bail!("tfy_tool_run did not return raw_ref");
    }
    let listed = child.request(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"tfy_scope_list","arguments":{"path":sample.to_str().unwrap(),"query":"add","limit":5}}}))?;
    let listed_json = mcp_content_json(&listed)?;
    let scope_id = listed_json["scopes"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|s| s["id"].as_str())
        .ok_or_else(|| anyhow!("smoke scope not found"))?
        .to_string();
    let context = child.request(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"tfy_context_get","arguments":{"session":"smoke","path":sample.to_str().unwrap(),"scope":scope_id,"compactness":"symbol"}}}))?;
    let compact = mcp_content_json(&context)?;
    let restore_payload = json!({
        "scope_id": compact["scope"]["id"],
        "language": compact["scope"]["language"],
        "compactness": compact["compactness"],
        "compact_code": "function f1(a,b){const c=a+b;return c*2;}",
        "base_compact_code": compact["compact_code"],
        "context_ref": compact["context_ref"],
        "symbol_map": compact["symbol_map"],
        "apply_proof": compact["apply_proof"]
    });
    let before_preview = fs::read_to_string(&sample)?;
    let preview = child.request(json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"tfy_output_validate","arguments":{"session":"smoke","restore_payload":restore_payload.clone()}}}))?;
    let preview_json = mcp_content_json(&preview)?;
    if preview_json["applied"] != false {
        bail!("preview unexpectedly applied");
    }
    if fs::read_to_string(&sample)? != before_preview {
        bail!("preview mutated file");
    }
    let applied = child.request(json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"tfy_output_apply","arguments":{"session":"smoke","restore_payload":restore_payload}}}))?;
    let applied_json = mcp_content_json(&applied)?;
    if applied_json["applied"] != true {
        bail!("apply did not report applied");
    }
    let after = fs::read_to_string(&sample)?;
    if !(after.contains("return total*2;") || after.contains("return total * 2;")) {
        bail!("apply result missing expected edit: {after}");
    }
    child.request(json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"tfy_state_project","arguments":{"session":"smoke"}}}))?;
    let events = load_events(&ledger).context("read smoke MCP ledger")?;
    Ok(SmokeReport {
        status: "pass".into(),
        mode: "mcp".into(),
        sample_path: sample.display().to_string(),
        scope_id,
        preview_applied: false,
        apply_applied: true,
        ledger_events: events.len(),
        evidence: vec![
            "MCP initialize and tools/list succeeded".into(),
            "tfy_tool_run stored raw command output and returned raw_ref".into(),
            "tfy_scope_list returned an exact scope id".into(),
            "tfy_context_get returned compact code, symbol map, context_ref, and ApplyProof".into(),
            "tfy_output_validate did not mutate the file".into(),
            "tfy_output_apply mutated only through proof-gated apply".into(),
            "tfy_state_project produced state projection evidence".into(),
            format!("ledger={}", ledger.display()),
            format!("raw_dir={}", raw.display()),
            format!("ledger_events={}", events.len()),
        ],
    })
}

fn print_codex_smoke_checklist() -> Result<()> {
    write_codex_smoke_checklist(std::io::stdout())
}

fn write_codex_smoke_checklist(mut output: impl Write) -> Result<()> {
    writeln!(
        output,
        "TFY Codex smoke is checklist/report-only in P0; it does not claim Codex host invocation."
    )?;
    writeln!(
        output,
        "1. Run `tfy init --codex --apply` and configure Codex MCP with the printed command."
    )?;
    writeln!(output, "2. Restart Codex.")?;
    writeln!(
        output,
        "3. Ask Codex to use TFY MCP only on a sample file: scope_list -> context_get -> output_validate -> output_apply."
    )?;
    writeln!(
        output,
        "4. Verify `.tfy/mcp/ledger.jsonl` contains context, validate, and apply events."
    )?;
    Ok(())
}

fn build_gain_report(ledgers: &[PathBuf], session: Option<&str>) -> Result<GainReport> {
    let mut report = GainReport {
        status: "pass".into(),
        ledgers: ledgers.iter().map(|p| p.display().to_string()).collect(),
        ..Default::default()
    };
    let mut sessions = BTreeMap::<String, ()>::new();
    for ledger in ledgers {
        let Ok(events) = load_events(ledger) else {
            continue;
        };
        for event in events {
            if let Some(session_filter) = session {
                if event.session_id != session_filter {
                    continue;
                }
            }
            sessions.insert(event.session_id.clone(), ());
            if let GatewayEvent::ToolCommandCompleted {
                command,
                exit_code,
                command_family,
                raw_bytes,
                model_bytes,
                raw_chars,
                summary_chars,
                model_chars,
                rendering_kind,
                ..
            } = event.payload
            {
                report.commands += 1;
                if exit_code != 0 {
                    report.failures += 1;
                }
                let raw_size = if raw_bytes == 0 { raw_chars } else { raw_bytes };
                let model_size = if model_bytes != 0 {
                    model_bytes
                } else if model_chars != 0 {
                    model_chars
                } else {
                    summary_chars
                };
                report.raw_bytes += raw_size;
                report.model_bytes += model_size;
                let rendering = if rendering_kind.is_empty() {
                    "legacy_unknown".to_string()
                } else {
                    rendering_kind
                };
                *report.rendering_counts.entry(rendering).or_insert(0) += 1;
                let family = if command_family.is_empty() {
                    tfy_core::classify_command_family(&command)
                } else {
                    command_family
                };
                *report.family_counts.entry(family).or_insert(0) += 1;
            }
        }
    }
    report.sessions = sessions.into_keys().collect();
    report.saved_bytes = report.raw_bytes as isize - report.model_bytes as isize;
    report.estimated_raw_tokens = estimate_tokens(report.raw_bytes);
    report.estimated_model_tokens = estimate_tokens(report.model_bytes);
    report.estimated_saved_tokens =
        report.estimated_raw_tokens as isize - report.estimated_model_tokens as isize;
    report.savings_pct = if report.raw_bytes == 0 {
        0.0
    } else {
        report.saved_bytes as f64 / report.raw_bytes as f64 * 100.0
    };
    let fallback_count: usize = report
        .rendering_counts
        .iter()
        .filter(|(kind, _)| {
            matches!(
                kind.as_str(),
                "pass_through" | "raw" | "fallback" | "legacy_unknown"
            )
        })
        .map(|(_, count)| *count)
        .sum();
    report.fallback_frequency = if report.commands == 0 {
        0.0
    } else {
        fallback_count as f64 / report.commands as f64
    };
    if report.commands == 0 {
        report.message = Some(NO_DATA_MESSAGE.into());
    }
    Ok(report)
}

fn gain_ledgers(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut ledgers = vec![
        PathBuf::from(".tfy/mcp/ledger.jsonl"),
        PathBuf::from(".tfy/adapter/ledger.jsonl"),
    ];
    ledgers.extend(extra.iter().cloned());
    ledgers
}

fn estimate_tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "pass" => "✓",
        "warn" => "!",
        _ => "✗",
    }
}

fn ensure_writable_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let probe = path.join(".tfy-write-test");
    fs::write(&probe, b"ok")?;
    let _ = fs::remove_file(probe);
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

struct McpChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpChild {
    fn request(&mut self, value: serde_json::Value) -> Result<serde_json::Value> {
        writeln!(self.stdin, "{}", value)?;
        self.stdin.flush()?;
        let mut line = String::new();
        self.stdout.read_line(&mut line)?;
        if !line.trim_start().starts_with('{') {
            bail!("MCP stdout pollution or no response: {line:?}");
        }
        Ok(serde_json::from_str(&line)?)
    }
}

impl Drop for McpChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_mcp_child(session: &str) -> Result<McpChild> {
    let root = std::env::temp_dir().join(format!(
        "tfy-doctor-{}-{}",
        std::process::id(),
        stable_id(session)
    ));
    fs::create_dir_all(&root)?;
    start_mcp_child_with_paths(session, &root.join("ledger.jsonl"), &root.join("raw"))
}

fn start_mcp_child_with_paths(session: &str, ledger: &Path, raw: &Path) -> Result<McpChild> {
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let mut child = Command::new(exe)
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "mcp",
            "serve",
            "--session",
            session,
            "--ledger",
            ledger
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 ledger path"))?,
            "--raw-dir",
            raw.to_str().ok_or_else(|| anyhow!("non-utf8 raw path"))?,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start tfy mcp serve")?;
    Ok(McpChild {
        stdin: child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("missing mcp stdin"))?,
        stdout: BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("missing mcp stdout"))?,
        ),
        child,
    })
}

fn tool_names(response: &serde_json::Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["name"].as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn mcp_content_json(response: &serde_json::Value) -> Result<serde_json::Value> {
    let text = response["result"]["content"]
        .as_array()
        .and_then(|content| content.first())
        .and_then(|content| content["text"].as_str())
        .ok_or_else(|| anyhow!("missing MCP text content: {response}"))?;
    Ok(serde_json::from_str(text)?)
}
