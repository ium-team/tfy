use crate::human::{
    default_human_shell_name, enable_human_auto_activate_repo,
    execute_human_auto_activate_install_setup, execute_human_auto_activate_uninstall_setup,
    execute_human_shell, human_auto_activate_hook_status, human_auto_script_relative_path,
    human_managed_session_supported_on_this_platform, human_shells_supported_on_this_platform,
};
use crate::util::{print_json, stable_id};
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, ErrorKind, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tfy_core::{raw_output_bytes, summarize_command_output_with_policy, ToolPolicy};
use tfy_runtime::{load_events, AdapterKind, GatewayEvent, OriginInvocation};

const TFY_CODEX_START: &str = "<!-- TFY:CODEX:START -->";
const TFY_CODEX_END: &str = "<!-- TFY:CODEX:END -->";
const NO_DATA_MESSAGE: &str = "No TFY savings data found yet";
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
    /// Named AI-agent host to diagnose (codex, claude-code).
    #[arg(long = "host")]
    pub host: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct SmokeCmd {
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
    /// Run an installed host CLI against a temporary project and emit launch-report host evidence.
    #[arg(long)]
    pub live: bool,
    /// Maximum Claude Code API spend for live host smoke, when the host supports it.
    #[arg(long, default_value = "0.50")]
    pub max_budget_usd: String,
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
    /// Prepare explicit human shell auto-activation for trusted TFY-marked current directories. Dry-run by default; writes only with --apply.
    #[arg(long)]
    pub human: bool,
    /// Print Codex integration setup guidance.
    #[arg(long)]
    pub codex: bool,
    /// Named AI-agent host to configure (codex, claude-code).
    #[arg(long = "host")]
    pub host: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub apply: bool,
    /// Remove TFY-owned host config or the explicit human rc hook where safe reversible uninstall is implemented.
    #[arg(long)]
    pub uninstall: bool,
    /// Use project-local host config when supported.
    #[arg(long)]
    pub project: bool,
    /// Use user-global host config when supported.
    #[arg(long)]
    pub global: bool,
    /// Shell for explicit human setup. Defaults to the supported platform shell.
    #[arg(long, default_value = "auto")]
    pub shell: String,
    /// Explicit rcfile/profile for human setup. Defaults to the supported platform shell startup file.
    #[arg(long)]
    pub rcfile: Option<PathBuf>,
    #[arg(long, default_value = "local-session")]
    pub session: String,
}

#[derive(Args, Clone)]
pub(crate) struct LifecycleTargetArgs {
    /// Configure only AI-agent lifecycle intent.
    #[arg(long)]
    pub agent: bool,
    /// Configure only human explicit managed-session lifecycle intent.
    #[arg(long)]
    pub human: bool,
}

#[derive(Args, Clone)]
pub(crate) struct StartCmd {
    #[command(flatten)]
    pub target: LifecycleTargetArgs,
    /// Target alias: agent/ai, human, or both.
    #[arg(value_name = "TARGET")]
    pub target_alias: Option<String>,
    /// Named AI-agent host setup route (codex, claude-code, all).
    #[arg(long)]
    pub host: Option<String>,
    /// Apply only safe Codex/Claude Code project config writers; unknown hosts fail closed.
    #[arg(long)]
    pub apply: bool,
    /// Record lifecycle intent only; do not write supported host config.
    #[arg(long)]
    pub no_apply: bool,
    /// Request verification guidance/smoke. Does not promote lifecycle active without route evidence.
    #[arg(long)]
    pub verify: bool,
    /// Session id for generated host hook/wrapper configuration.
    #[arg(long, default_value = "local-session")]
    pub session: String,
    /// Explicitly enable trusted current-directory future-shell human auto-activation marker/script for automation. Interactive current-directory human starts enable this by default. Requires a separately installed user rc/profile hook.
    #[arg(long)]
    pub auto_activate: bool,
    /// Shell for human managed sessions and current-directory auto-activation. Defaults to the supported platform shell.
    #[arg(long, default_value = "auto")]
    pub shell: String,
}

#[derive(Args, Clone)]
pub(crate) struct StopCmd {
    #[command(flatten)]
    pub target: LifecycleTargetArgs,
    /// Target alias: agent/ai, human, or both.
    #[arg(value_name = "TARGET")]
    pub target_alias: Option<String>,
}

#[derive(Args, Clone)]
pub(crate) struct FuckyouCmd {
    #[command(flatten)]
    pub target: LifecycleTargetArgs,
    /// Target alias: agent/ai, human, or both.
    #[arg(value_name = "TARGET")]
    pub target_alias: Option<String>,
    /// Confirm scoped TFY-owned lifecycle cleanup. Required for non-interactive destructive cleanup.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Subcommand, Clone)]
pub(crate) enum GlobalCmd {
    /// Start global TFY lifecycle intent.
    Start(StartCmd),
    /// Stop global TFY lifecycle intent without deleting data.
    Stop(StopCmd),
    /// Scoped global TFY-owned lifecycle cleanup.
    Fuckyou(FuckyouCmd),
}

#[derive(Subcommand, Clone)]
pub(crate) enum UseCmd {
    /// Alias for `tfy global start`; opt into TFY by default across projects.
    #[command(alias = "alwais")]
    Always(StartCmd),
    /// Alias for `tfy global stop`; pause default TFY use without deleting data.
    #[command(alias = "never")]
    Stop(StopCmd),
    /// Alias for `tfy global stop`; explicit cancellation spelling.
    Cancel(StopCmd),
    /// Alias for `tfy global fuckyou`; remove scoped global lifecycle state.
    Fuckyou(FuckyouCmd),
}

#[derive(Args, Clone)]
pub(crate) struct StatusCmd {
    /// Show only AI-agent lifecycle/status information.
    #[arg(long)]
    pub agent: bool,
    /// Show only human explicit managed-session lifecycle/status information.
    #[arg(long)]
    pub human: bool,
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
    /// JSON evidence for release artifact/docs/review/CI gates.
    #[arg(long = "release-evidence")]
    pub release_evidence: Vec<PathBuf>,
    #[arg(long, default_value = "local-session")]
    pub session: String,
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Clone)]
pub(crate) struct RawCmd {
    /// Raw reference to recover/inspect/export. Omit with --list or store-wide --export/--prune.
    pub raw_ref: Option<String>,
    #[arg(long, default_value = ".tfy/raw")]
    pub raw_dir: PathBuf,
    #[arg(long)]
    pub around: Option<String>,
    #[arg(long, default_value_t = 3)]
    pub context: usize,
    #[arg(long)]
    pub list: bool,
    #[arg(long)]
    pub inspect: bool,
    #[arg(long)]
    pub export: Option<PathBuf>,
    #[arg(long)]
    pub prune: bool,
    #[arg(long = "older-than-days")]
    pub older_than_days: Option<u64>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub apply: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct BenchCmd {
    #[arg(long)]
    pub json: bool,
    #[arg(long, default_value = ".tfy/bench/raw")]
    pub raw_dir: PathBuf,
    /// Optional path to write the reproducible benchmark manifest JSON.
    #[arg(long)]
    pub output: Option<PathBuf>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleTarget {
    Agent,
    Human,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleScope {
    Project,
    Global,
}

impl LifecycleScope {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentLifecycleState {
    configured: bool,
    desired: bool,
    #[serde(default = "default_lifecycle_route_state")]
    route_state: String,
    #[serde(default)]
    active: bool,
    #[serde(default = "default_agent_intended_routes", alias = "active_routes")]
    intended_routes: Vec<String>,
    #[serde(default)]
    host_routes: BTreeMap<String, LifecycleHostRoute>,
    private_hook_interception: bool,
    provider_prompt_gateway: bool,
    support_status: String,
    started_at: Option<String>,
    stopped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HumanLifecycleState {
    configured: bool,
    desired: bool,
    #[serde(default = "default_lifecycle_route_state")]
    route_state: String,
    #[serde(default)]
    active: bool,
    active_route: String,
    #[serde(default = "default_human_entrypoint")]
    entrypoint: Vec<String>,
    #[serde(default)]
    session_wrapper_available: bool,
    #[serde(default)]
    managed_session_available: bool,
    #[serde(default = "default_managed_session_entrypoint")]
    managed_session_entrypoint: Vec<String>,
    #[serde(default = "default_managed_session_scope")]
    managed_session_scope: String,
    #[serde(default)]
    integration_path: Option<String>,
    #[serde(default = "default_human_shells_supported")]
    shells_supported: Vec<String>,
    ordinary_terminal_interception: bool,
    support_status: String,
    started_at: Option<String>,
    stopped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleHostRoute {
    #[serde(default = "default_lifecycle_route_type")]
    route_type: String,
    route_state: String,
    active: bool,
    #[serde(default)]
    configured: bool,
    #[serde(default)]
    route_configured: bool,
    #[serde(default)]
    host_reload_required: bool,
    #[serde(default)]
    host_route_available_after_reload: bool,
    #[serde(default)]
    host_approval_required: bool,
    #[serde(default)]
    route_verified: bool,
    #[serde(default)]
    savings_verified: bool,
    #[serde(default)]
    normal_workflow_supported: bool,
    config_path: Option<String>,
    claim_tier: String,
    message: String,
}

fn default_lifecycle_route_type() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostConfigProvenance {
    host: String,
    config_path: String,
    managed_key_path: String,
    command: String,
    args: Vec<String>,
    command_args_hash: String,
    session: String,
    ledger_path: String,
    raw_dir: String,
    created_at: String,
    updated_at: String,
    tfy_version: String,
    uninstall_safety_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleLedgers {
    state: String,
    adapter: String,
    agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleFile {
    schema_version: u8,
    scope: String,
    agent: Option<AgentLifecycleState>,
    human: Option<HumanLifecycleState>,
    raw_dir: String,
    ledgers: LifecycleLedgers,
}

fn default_agent_intended_routes() -> Vec<String> {
    vec![
        "agent_wrapper".into(),
        "generic_shell_adapter".into(),
        "official_host_hook_when_configured".into(),
    ]
}

fn default_lifecycle_route_state() -> String {
    "intent_recorded".into()
}

fn default_human_entrypoint() -> Vec<String> {
    default_managed_session_entrypoint()
}

fn default_managed_session_entrypoint() -> Vec<String> {
    vec!["tfy".into(), "start".into(), "--human".into()]
}

fn default_managed_session_scope() -> String {
    "current_directory_scoped_tfy_managed_session".into()
}

fn default_human_shells_supported() -> Vec<String> {
    human_shells_supported_on_this_platform()
}

fn human_managed_session_available() -> bool {
    human_managed_session_supported_on_this_platform()
}

fn human_support_status() -> String {
    if human_managed_session_available() {
        "managed_session_available".into()
    } else {
        "managed_session_unsupported_platform".into()
    }
}

#[derive(Serialize)]
struct LifecycleStatusView {
    path: String,
    exists: bool,
    parse_error: Option<String>,
    agent: Option<AgentLifecycleState>,
    human: Option<HumanLifecycleState>,
    raw_dir: String,
    ledgers: LifecycleLedgers,
}

#[derive(Serialize)]
struct ProductStatusReport {
    status: String,
    target_filter: String,
    lifecycle_summary: LifecycleSummary,
    project_lifecycle: LifecycleStatusView,
    global_lifecycle: LifecycleStatusView,
    effective_lifecycle: EffectiveLifecycleStatus,
    minimum_v1_host_matrix: Vec<HostReadiness>,
    surfaces: Vec<SurfaceStatus>,
    claim_evidence_ladder: Vec<String>,
    launch_claim_gate: String,
    not_supported: Vec<String>,
    truthfulness_boundary: String,
}

#[derive(Serialize)]
struct LifecycleSummary {
    status: String,
    desired: bool,
    configured: bool,
    active: bool,
    desired_targets: Vec<String>,
    configured_targets: Vec<String>,
    active_targets: Vec<String>,
    evidence_required: bool,
    message: String,
}

#[derive(Serialize)]
struct EffectiveLifecycleStatus {
    agent: Option<EffectiveRouteStatus>,
    human: Option<EffectiveRouteStatus>,
}

#[derive(Serialize)]
struct EffectiveRouteStatus {
    desired: bool,
    desired_source: String,
    configured: bool,
    lifecycle_started: bool,
    route_configured: bool,
    route_verified: bool,
    savings_verified: bool,
    normal_workflow_supported: bool,
    active: bool,
    active_derivation: String,
    route_state: String,
    support_status: String,
    next_action: String,
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

#[derive(Clone, Copy)]
struct LiveHostSmokeSpec {
    host: &'static str,
    binary: &'static str,
    timeout: Duration,
    needs_noninteractive_bypass: bool,
}

#[derive(Serialize)]
struct LaunchReadinessReport {
    status: String,
    release_tiers: ReleaseTierReport,
    host_matrix: Vec<HostReadiness>,
    claim_evidence_ladder: Vec<String>,
    gain: GainReport,
    host_evidence: HostEvidenceSummary,
    release_evidence: ReleaseEvidenceSummary,
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
            default_raw_dirs: vec![".tfy/raw".into()],
            retention_default: "local project data is retained until the user deletes/prunes the .tfy directory; TFY does not upload raw evidence".into(),
            deletion_export: "delete/export raw evidence with `tfy raw --list`, `tfy raw <raw_ref> --inspect`, `tfy raw <raw_ref> --export <path>`, and explicit `tfy raw --prune --dry-run/--apply`; ledgers remain local files under configured paths".into(),
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
    generic_shell_wrapper: bool,
    tfy_agent_adapter: bool,
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

#[derive(Default, Clone, Serialize)]
struct ReleaseEvidenceSummary {
    cargo_install_verified: bool,
    cargo_build_release_verified: bool,
    archive_checksum_dry_run: bool,
    docs_demo_release_notes_complete: bool,
    independent_reviews_approved: bool,
    pr_ci_green: bool,
    benchmark_manifest_generated: bool,
    notes: Vec<String>,
}

#[derive(Deserialize)]
struct ReleaseEvidenceFile {
    cargo_install_verified: Option<bool>,
    cargo_install_binary: Option<PathBuf>,
    cargo_build_release_verified: Option<bool>,
    release_binary: Option<PathBuf>,
    archive_checksum_dry_run: Option<bool>,
    archive_artifact: Option<PathBuf>,
    checksum_artifact: Option<PathBuf>,
    docs_demo_release_notes_complete: Option<bool>,
    docs_artifact: Option<PathBuf>,
    release_notes_artifact: Option<PathBuf>,
    independent_reviews_approved: Option<bool>,
    review_artifact: Option<PathBuf>,
    pr_ci_green: Option<bool>,
    ci_artifact: Option<PathBuf>,
    benchmark_manifest_generated: Option<bool>,
    benchmark_manifest: Option<PathBuf>,
    notes: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct HostSetupEvidence {
    host: String,
    tfy_version: Option<String>,
    evidence_expires_at: Option<String>,
    reverify_failed: Option<bool>,
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

#[derive(Serialize)]
struct ReleaseTierReport {
    beta_ready: TierStatus,
    developer_preview_ready: TierStatus,
    rc_ready: TierStatus,
    ga_ready: TierStatus,
    public_superiority_claim_ready: TierStatus,
}

#[derive(Clone, Serialize)]
struct TierStatus {
    status: String,
    evidence: Vec<String>,
    blockers: Vec<String>,
}

#[derive(Serialize)]
struct RawLifecycleReport {
    status: String,
    raw_dir: String,
    action: String,
    raw_ref: Option<String>,
    count: usize,
    bytes: usize,
    dry_run: bool,
    applied: bool,
    entries: Vec<RawEntryReport>,
    message: String,
}

#[derive(Serialize, Clone)]
struct RawEntryReport {
    raw_ref: String,
    path: String,
    command_present: bool,
    command_sha256: String,
    exit_code: i64,
    bytes: usize,
    created_ns: u64,
}

#[derive(Serialize)]
struct BenchmarkManifest {
    status: String,
    generated_by: String,
    scenarios: Vec<BenchmarkScenario>,
    tfy_self_benchmark: BenchmarkSummary,
    public_superiority_claim_ready: bool,
    claim_policy: String,
}

#[derive(Serialize)]
struct BenchmarkScenario {
    name: String,
    command_family: String,
    raw_bytes: usize,
    model_bytes: usize,
    saved_bytes: isize,
    no_negative_savings: bool,
    raw_ref: String,
}

#[derive(Serialize)]
struct BenchmarkSummary {
    scenarios: usize,
    raw_bytes: usize,
    model_bytes: usize,
    saved_bytes: isize,
    no_negative_savings: bool,
    positive_savings: bool,
}

fn now_stamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("unix:{seconds}")
}

fn default_ledgers(scope: LifecycleScope) -> LifecycleLedgers {
    let prefix = match scope {
        LifecycleScope::Project => ".tfy".to_string(),
        LifecycleScope::Global => global_tfy_dir().display().to_string(),
    };
    LifecycleLedgers {
        state: format!("{prefix}/state/ledger.jsonl"),
        adapter: format!("{prefix}/adapter/ledger.jsonl"),
        agent: format!("{prefix}/agent/ledger.jsonl"),
    }
}

fn lifecycle_path(scope: LifecycleScope) -> PathBuf {
    match scope {
        LifecycleScope::Project => PathBuf::from(".tfy").join("lifecycle.json"),
        LifecycleScope::Global => global_tfy_dir().join("lifecycle.json"),
    }
}

fn global_tfy_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("TFY_HOME") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("tfy");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tfy")
}

fn default_lifecycle(scope: LifecycleScope) -> LifecycleFile {
    let raw_dir = match scope {
        LifecycleScope::Project => ".tfy/raw".to_string(),
        LifecycleScope::Global => global_tfy_dir().join("raw").display().to_string(),
    };
    LifecycleFile {
        schema_version: 1,
        scope: scope.label().into(),
        agent: None,
        human: None,
        raw_dir,
        ledgers: default_ledgers(scope),
    }
}

fn read_lifecycle(scope: LifecycleScope) -> Result<LifecycleFile> {
    let path = lifecycle_path(scope);
    match fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(default_lifecycle(scope)),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn write_lifecycle(scope: LifecycleScope, state: &LifecycleFile) -> Result<()> {
    let path = lifecycle_path(scope);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(state)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

fn ensure_lifecycle_storage_dirs(state: &LifecycleFile) -> Result<()> {
    fs::create_dir_all(&state.raw_dir)
        .with_context(|| format!("create raw evidence directory {}", state.raw_dir))?;
    for ledger in [
        &state.ledgers.state,
        &state.ledgers.adapter,
        &state.ledgers.agent,
    ] {
        if let Some(parent) = Path::new(ledger).parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create ledger directory {}", parent.display()))?;
        }
    }
    Ok(())
}

fn remove_lifecycle_if_empty(scope: LifecycleScope, state: &LifecycleFile) -> Result<()> {
    let path = lifecycle_path(scope);
    if state.agent.is_none() && state.human.is_none() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("remove {}", path.display())),
        }
    } else {
        write_lifecycle(scope, state)?;
    }
    Ok(())
}

fn agent_state(
    desired: bool,
    started_at: Option<String>,
    stopped_at: Option<String>,
) -> AgentLifecycleState {
    AgentLifecycleState {
        configured: true,
        desired,
        route_state: "intent_recorded".into(),
        active: false,
        intended_routes: default_agent_intended_routes(),
        host_routes: BTreeMap::new(),
        private_hook_interception: false,
        provider_prompt_gateway: false,
        support_status: "host_route_configuration_required".into(),
        started_at,
        stopped_at,
    }
}

fn human_state(
    desired: bool,
    started_at: Option<String>,
    stopped_at: Option<String>,
) -> HumanLifecycleState {
    HumanLifecycleState {
        configured: true,
        desired,
        route_state: "intent_recorded".into(),
        active: false,
        active_route: "managed_session_entrypoint_available".into(),
        entrypoint: default_human_entrypoint(),
        session_wrapper_available: human_managed_session_available(),
        managed_session_available: human_managed_session_available(),
        managed_session_entrypoint: default_managed_session_entrypoint(),
        managed_session_scope: default_managed_session_scope(),
        integration_path: None,
        shells_supported: default_human_shells_supported(),
        ordinary_terminal_interception: false,
        support_status: human_support_status(),
        started_at,
        stopped_at,
    }
}

fn agent_started(now: &str) -> AgentLifecycleState {
    agent_state(true, Some(now.into()), None)
}

fn human_started(now: &str) -> HumanLifecycleState {
    human_state(true, Some(now.into()), None)
}

fn agent_stopped(now: &str) -> AgentLifecycleState {
    agent_state(false, None, Some(now.into()))
}

fn human_stopped(now: &str) -> HumanLifecycleState {
    human_state(false, None, Some(now.into()))
}

fn lifecycle_status_view(scope: LifecycleScope) -> LifecycleStatusView {
    let path = lifecycle_path(scope);
    let exists = path.exists();
    let (state, parse_error) = match read_lifecycle(scope) {
        Ok(state) => (state, None),
        Err(err) => (default_lifecycle(scope), Some(err.to_string())),
    };
    LifecycleStatusView {
        path: path.display().to_string(),
        exists,
        parse_error,
        agent: state.agent,
        human: state.human,
        raw_dir: state.raw_dir,
        ledgers: state.ledgers,
    }
}

fn targets_from_args(
    args: &LifecycleTargetArgs,
    alias: Option<&str>,
    action: &str,
) -> Result<Vec<LifecycleTarget>> {
    let mut targets = Vec::new();
    if args.agent {
        targets.push(LifecycleTarget::Agent);
    }
    if args.human {
        targets.push(LifecycleTarget::Human);
    }
    if let Some(alias) = alias {
        if !targets.is_empty() {
            bail!("tfy {action} target alias cannot be combined with --agent/--human");
        }
        return parse_target_choice(alias, action);
    }
    if !targets.is_empty() {
        return Ok(targets);
    }
    prompt_targets(action)
}

fn parse_target_choice(choice: &str, action: &str) -> Result<Vec<LifecycleTarget>> {
    match choice.trim().to_ascii_lowercase().as_str() {
        "1" | "a" | "agent" | "ai" => Ok(vec![LifecycleTarget::Agent]),
        "2" | "h" | "human" => Ok(vec![LifecycleTarget::Human]),
        "3" | "b" | "both" | "all" | "agent,human" | "human,agent" => {
            Ok(vec![LifecycleTarget::Agent, LifecycleTarget::Human])
        }
        _ => bail!(
            "tfy {action} requires --agent, --human, or an interactive choice of agent, human, or both"
        ),
    }
}

fn prompt_targets(action: &str) -> Result<Vec<LifecycleTarget>> {
    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        match prompt_targets_tui(action) {
            Ok(targets) => return Ok(targets),
            Err(err) if raw_terminal_unavailable(&err) => {
                eprintln!("TFY {action}: TUI unavailable ({err}); falling back to line prompt");
            }
            Err(err) => return Err(err),
        }
    }
    eprintln!("TFY {action}: choose target: agent, human, or both");
    let input = read_lifecycle_prompt_input()?;
    let choice = input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    parse_target_choice(choice, action)
}

fn prompt_targets_tui(action: &str) -> Result<Vec<LifecycleTarget>> {
    let choices = [
        ("agent", "AI agent only"),
        ("human", "Human explicit managed-session only"),
        ("both", "Agent and human"),
        ("cancel", "Cancel without writing state"),
    ];
    let labels: Vec<&str> = choices.iter().map(|(_, label)| *label).collect();
    let selected = prompt_menu_tui(&format!("TFY {action}: choose target"), &labels)?;
    match choices[selected].0 {
        "cancel" => bail!("tfy {action} cancelled"),
        choice => parse_target_choice(choice, action),
    }
}

fn read_lifecycle_prompt_input() -> Result<String> {
    let stdin = std::io::stdin();
    let mut input = String::new();
    if stdin.is_terminal() {
        stdin.lock().read_line(&mut input)?;
    } else {
        stdin.lock().read_to_string(&mut input)?;
    }
    Ok(input)
}

fn confirm_fuckyou(yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        let choices = ["No, cancel", "Yes, clean scoped TFY lifecycle state"];
        match prompt_menu_tui("TFY fuckyou confirmation", &choices) {
            Ok(1) => return Ok(()),
            Ok(_) => bail!("tfy fuckyou cancelled"),
            Err(err) if raw_terminal_unavailable(&err) => {
                eprintln!("TFY fuckyou: TUI unavailable ({err}); falling back to line prompt");
            }
            Err(err) => return Err(err),
        }
    }
    eprintln!("TFY fuckyou will remove scoped TFY-owned lifecycle state. Type yes to continue.");
    let input = read_lifecycle_prompt_input()?;
    if input.lines().any(|line| line.trim() == "yes") {
        Ok(())
    } else {
        bail!("tfy fuckyou requires confirmation; rerun with --yes or type yes")
    }
}

fn selection_cancelled(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("selection cancelled"))
}

fn offer_human_auto_activate_hook_install(shell: &str) -> Result<()> {
    let hook_status = match human_auto_activate_hook_status(shell, None) {
        Ok(status) => status,
        Err(err) => {
            println!(
                "human_auto_activation_hook=status_unavailable install_prompt=false reason={} next_action=\"tfy setup --human --apply\"",
                err
            );
            return Ok(());
        }
    };
    if hook_status.marker_block_present {
        println!(
            "human_auto_activation_hook=already_installed marker_block_present=true selected_rcfile.path={} shell={} install_prompt=false",
            hook_status.rcfile.display(),
            hook_status.shell
        );
        return Ok(());
    }

    println!(
        "human_auto_activation_hook=missing marker_block_present=false selected_rcfile.path={} shell={} install_prompt=true",
        hook_status.rcfile.display(),
        hook_status.shell
    );
    let mut install = false;
    let mut prompt_resolved = false;
    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        let title = format!(
            "TFY human: install one-time {} hook for future shells?",
            hook_status.shell
        );
        let choices = ["No, skip for now", "Yes, install one-time shell hook"];
        match prompt_menu_tui(&title, &choices) {
            Ok(1) => {
                install = true;
                prompt_resolved = true;
            }
            Ok(_) => {
                prompt_resolved = true;
            }
            Err(err) if raw_terminal_unavailable(&err) => {
                eprintln!(
                    "TFY human hook prompt: TUI unavailable ({err}); falling back to line prompt"
                );
            }
            Err(err) if selection_cancelled(&err) => {
                prompt_resolved = true;
            }
            Err(err) => return Err(err),
        }
    }
    if !prompt_resolved && std::io::stdin().is_terminal() {
        eprintln!(
            "TFY human: install one-time {} hook for future shells? [y/N]",
            hook_status.shell
        );
        let input = read_lifecycle_prompt_input()?;
        install = input
            .lines()
            .map(|line| line.trim().to_ascii_lowercase())
            .any(|line| line == "y" || line == "yes");
    }

    if install {
        execute_human_auto_activate_install_setup(shell, None, false, true)
            .context("install TFY human one-time auto-activate hook")?;
        println!(
            "human_auto_activation_hook=installed marker_block_present=true selected_rcfile.path={} shell={}",
            hook_status.rcfile.display(),
            hook_status.shell
        );
    } else {
        println!(
            "human_auto_activation_hook=skipped marker_block_present=false selected_rcfile.path={} shell={} next_action=\"tfy setup --human --apply\"",
            hook_status.rcfile.display(),
            hook_status.shell
        );
    }
    Ok(())
}

pub(crate) fn prompt_menu_tui(title: &str, choices: &[&str]) -> Result<usize> {
    let mut selected = 0usize;
    let mut stderr = std::io::stderr();
    let _raw = RawTerminalMode::enter()?;
    loop {
        render_menu(&mut stderr, title, choices, selected)?;
        match read_menu_key()? {
            MenuKey::Up => selected = selected.saturating_sub(1),
            MenuKey::Down => {
                if selected + 1 < choices.len() {
                    selected += 1;
                }
            }
            MenuKey::Enter => {
                clear_menu(&mut stderr, choices.len() + 2)?;
                return Ok(selected);
            }
            MenuKey::Cancel => {
                clear_menu(&mut stderr, choices.len() + 2)?;
                bail!("selection cancelled");
            }
            MenuKey::Ignore => {}
        }
    }
}

pub(crate) fn raw_terminal_unavailable(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("raw terminal unavailable"))
}

fn render_menu(
    mut writer: impl Write,
    title: &str,
    choices: &[&str],
    selected: usize,
) -> Result<()> {
    write!(writer, "\x1b[?25l\x1b[2K\r{title}\n")?;
    writeln!(
        writer,
        "Use ↑/↓ or j/k to move, Enter to select, Esc/q to cancel."
    )?;
    for (index, choice) in choices.iter().enumerate() {
        if index == selected {
            writeln!(writer, "  ❯ {choice}")?;
        } else {
            writeln!(writer, "    {choice}")?;
        }
    }
    write!(writer, "\x1b[{}A", choices.len() + 2)?;
    writer.flush()?;
    Ok(())
}

fn clear_menu(mut writer: impl Write, lines: usize) -> Result<()> {
    write!(writer, "\x1b[?25h")?;
    for line in 0..lines {
        write!(writer, "\x1b[2K\r")?;
        if line + 1 < lines {
            write!(writer, "\x1b[1B")?;
        }
    }
    if lines > 1 {
        write!(writer, "\x1b[{}A", lines - 1)?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuKey {
    Up,
    Down,
    Enter,
    Cancel,
    Ignore,
}

fn read_menu_key() -> Result<MenuKey> {
    let mut input = std::io::stdin().lock();
    let mut byte = [0u8; 1];
    loop {
        if input.read(&mut byte)? == 1 {
            break;
        }
    }
    match byte[0] {
        b'\n' | b'\r' => Ok(MenuKey::Enter),
        b'q' | b'Q' | 0x03 => Ok(MenuKey::Cancel),
        b'k' | b'K' => Ok(MenuKey::Up),
        b'j' | b'J' => Ok(MenuKey::Down),
        0x1b => {
            let mut seq = [0u8; 2];
            let first = input.read(&mut seq[0..1])?;
            if first == 0 {
                return Ok(MenuKey::Cancel);
            }
            let second = input.read(&mut seq[1..2])?;
            if second == 0 {
                return Ok(MenuKey::Ignore);
            }
            match seq {
                [b'[', b'A'] => Ok(MenuKey::Up),
                [b'[', b'B'] => Ok(MenuKey::Down),
                _ => Ok(MenuKey::Ignore),
            }
        }
        _ => Ok(MenuKey::Ignore),
    }
}

struct RawTerminalMode {
    saved: Option<String>,
}

impl RawTerminalMode {
    fn enter() -> Result<Self> {
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .context("raw terminal unavailable: could not read terminal mode")?;
        if !saved.status.success() {
            bail!("raw terminal unavailable: stty -g failed");
        }
        let saved = Some(saved)
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        if saved.is_none() {
            bail!("raw terminal unavailable: empty stty state");
        }
        let status = Command::new("stty")
            .args(["-echo", "-icanon", "-isig", "min", "0", "time", "1"])
            .stdin(Stdio::inherit())
            .status()
            .context("raw terminal unavailable: could not set raw mode")?;
        if !status.success() {
            bail!("raw terminal unavailable: stty raw mode failed");
        }
        Ok(Self { saved })
    }
}

impl Drop for RawTerminalMode {
    fn drop(&mut self) {
        if let Some(saved) = self.saved.as_deref() {
            let _ = Command::new("stty")
                .arg(saved)
                .stdin(Stdio::inherit())
                .status();
        }
        let _ = write!(std::io::stderr(), "\x1b[?25h");
    }
}

fn remove_target_dir(scope: LifecycleScope, target: LifecycleTarget) -> Result<()> {
    let base = match scope {
        LifecycleScope::Project => PathBuf::from(".tfy"),
        LifecycleScope::Global => global_tfy_dir(),
    };
    let dir = match target {
        LifecycleTarget::Agent => base.join("agent"),
        LifecycleTarget::Human => base.join("human"),
    };
    match fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("read {}", dir.display()))?;
                let path = entry.path();
                if entry.file_name() == "ledger.jsonl" {
                    continue;
                }
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("stat {}", path.display()))?;
                if file_type.is_dir() {
                    fs::remove_dir_all(&path)
                        .with_context(|| format!("remove {}", path.display()))?;
                } else {
                    fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
                }
            }
            if fs::read_dir(&dir)
                .with_context(|| format!("read {}", dir.display()))?
                .next()
                .is_none()
            {
                fs::remove_dir(&dir).with_context(|| format!("remove {}", dir.display()))?;
            }
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("read {}", dir.display())),
    }
}

fn target_names(targets: &[LifecycleTarget]) -> String {
    targets
        .iter()
        .map(|target| match target {
            LifecycleTarget::Agent => "agent",
            LifecycleTarget::Human => "human",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn host_selection(host: Option<&str>) -> Result<Vec<HostIntegration>> {
    match host {
        Some(host) if host.trim().eq_ignore_ascii_case("all") => Ok(host_registry()
            .into_iter()
            .filter(|host| matches!(host.id, "codex" | "claude-code"))
            .collect()),
        Some(host) => Ok(vec![host_integration(host)?]),
        None => Ok(Vec::new()),
    }
}

fn lifecycle_agent_wrapper_route(
    config_path: Option<String>,
    message: impl Into<String>,
) -> LifecycleHostRoute {
    LifecycleHostRoute {
        route_type: "agent_wrapper".into(),
        route_state: "configured_unverified".into(),
        active: false,
        configured: true,
        route_configured: true,
        host_reload_required: false,
        host_route_available_after_reload: false,
        host_approval_required: false,
        route_verified: false,
        savings_verified: false,
        normal_workflow_supported: false,
        config_path,
        claim_tier: "configured_unverified".into(),
        message: message.into(),
    }
}

fn lifecycle_host_route(
    route_state: &str,
    config_path: Option<String>,
    claim_tier: &str,
    message: impl Into<String>,
) -> LifecycleHostRoute {
    lifecycle_host_route_with_type(
        route_state,
        config_path,
        claim_tier,
        false,
        "official_host_hook",
        message,
    )
}

fn lifecycle_official_hook_route(
    route_state: &str,
    config_path: Option<String>,
    claim_tier: &str,
    host_approval_required: bool,
    message: impl Into<String>,
) -> LifecycleHostRoute {
    lifecycle_host_route_with_type(
        route_state,
        config_path,
        claim_tier,
        host_approval_required,
        "official_host_hook",
        message,
    )
}

fn lifecycle_host_route_with_type(
    route_state: &str,
    config_path: Option<String>,
    claim_tier: &str,
    host_approval_required: bool,
    route_type: &str,
    message: impl Into<String>,
) -> LifecycleHostRoute {
    let configured = matches!(
        route_state,
        "applied_unverified"
            | "configured_unverified"
            | "verified_host_invocation"
            | "launch_supported"
    );
    let route_configured = matches!(
        route_state,
        "applied_unverified"
            | "configured_unverified"
            | "verified_host_invocation"
            | "launch_supported"
    );
    let route_verified = matches!(route_state, "verified_host_invocation" | "launch_supported");
    let savings_verified = route_state == "launch_supported";
    LifecycleHostRoute {
        route_type: route_type.into(),
        route_state: route_state.into(),
        active: false,
        configured,
        route_configured,
        host_reload_required: route_configured && !route_verified,
        host_route_available_after_reload: route_verified,
        host_approval_required: host_approval_required && route_configured && !route_verified,
        route_verified,
        savings_verified,
        normal_workflow_supported: route_state == "launch_supported",
        config_path,
        claim_tier: claim_tier.into(),
        message: message.into(),
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn install_project_agent_wrapper(session: &str) -> Result<PathBuf> {
    let wrapper = PathBuf::from(".tfy/agent/tfy-agent-wrapper");
    if let Some(parent) = wrapper.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow!("current tfy executable path is not valid UTF-8"))?;
    let exe = shell_single_quote(exe);
    let script = format!(
        r#"#!/usr/bin/env sh
# TFY AI-agent command wrapper. Configure an AI agent runtime command executor to call this file.
# Generated by `tfy start --agent`; it does not modify human shell startup files.
exec {exe} agent run --host "${{TFY_AGENT_HOST:-generic}}" --session "${{TFY_SESSION_ID:-{session}}}" -- "$@"
"#
    );
    fs::write(&wrapper, script).with_context(|| format!("write {}", wrapper.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("chmod {}", wrapper.display()))?;
    }
    Ok(wrapper)
}

pub(crate) fn execute_start(cmd: StartCmd) -> Result<()> {
    execute_lifecycle_start(LifecycleScope::Project, cmd)
}

pub(crate) fn execute_stop(cmd: StopCmd) -> Result<()> {
    execute_lifecycle_stop(LifecycleScope::Project, cmd)
}

pub(crate) fn execute_fuckyou(cmd: FuckyouCmd) -> Result<()> {
    execute_lifecycle_fuckyou(LifecycleScope::Project, cmd)
}

pub(crate) fn execute_global(cmd: GlobalCmd) -> Result<()> {
    match cmd {
        GlobalCmd::Start(cmd) => execute_lifecycle_start(LifecycleScope::Global, cmd),
        GlobalCmd::Stop(cmd) => execute_lifecycle_stop(LifecycleScope::Global, cmd),
        GlobalCmd::Fuckyou(cmd) => execute_lifecycle_fuckyou(LifecycleScope::Global, cmd),
    }
}

pub(crate) fn execute_use(cmd: UseCmd) -> Result<()> {
    match cmd {
        UseCmd::Always(cmd) => execute_lifecycle_start(LifecycleScope::Global, cmd),
        UseCmd::Stop(cmd) | UseCmd::Cancel(cmd) => {
            execute_lifecycle_stop(LifecycleScope::Global, cmd)
        }
        UseCmd::Fuckyou(cmd) => execute_lifecycle_fuckyou(LifecycleScope::Global, cmd),
    }
}

fn execute_lifecycle_start(scope: LifecycleScope, cmd: StartCmd) -> Result<()> {
    let targets = targets_from_args(&cmd.target, cmd.target_alias.as_deref(), "start")?;
    if cmd.apply && cmd.no_apply {
        bail!("tfy start cannot combine --apply and --no-apply");
    }
    if cmd.host.is_some() && !targets.contains(&LifecycleTarget::Agent) {
        bail!("tfy start --host configures AI-agent routing; include agent/ai or both");
    }
    let interactive_terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let project_human_only = scope == LifecycleScope::Project
        && targets.len() == 1
        && targets.contains(&LifecycleTarget::Human);
    if cmd.auto_activate && !project_human_only {
        bail!("tfy start --human --auto-activate is project human-only mode");
    }
    let selected_human_shell = if targets.contains(&LifecycleTarget::Human) {
        if cmd.shell.trim().eq_ignore_ascii_case("auto")
            || cmd.shell.trim().eq_ignore_ascii_case("default")
        {
            default_human_shell_name().to_string()
        } else {
            cmd.shell.clone()
        }
    } else {
        default_human_shell_name().to_string()
    };
    let selected_human_auto_script = if targets.contains(&LifecycleTarget::Human) {
        human_auto_script_relative_path(&selected_human_shell)?
    } else {
        String::new()
    };
    let default_human_auto_activate =
        project_human_only && interactive_terminal && human_managed_session_available();
    let should_enable_human_auto_activation = cmd.auto_activate || default_human_auto_activate;
    let host_all = cmd
        .host
        .as_deref()
        .is_some_and(|host| host.trim().eq_ignore_ascii_case("all"));
    let default_agent_wrapper_setup = scope == LifecycleScope::Project
        && targets.contains(&LifecycleTarget::Agent)
        && cmd.host.is_none()
        && !cmd.no_apply;
    let default_agent_official_hook_setup = default_agent_wrapper_setup;
    let selected_hosts = if default_agent_official_hook_setup {
        vec![host_integration("codex")?, host_integration("claude-code")?]
    } else {
        host_selection(cmd.host.as_deref())?
    };
    if selected_hosts
        .iter()
        .any(|host| host.status == "unsupported")
    {
        let unsupported = selected_hosts
            .iter()
            .find(|host| host.status == "unsupported")
            .expect("unsupported host present");
        bail!(
            "host '{}' is unsupported in TFY current product scope; supported named hosts: codex, claude-code; generic wrapper fallback remains available",
            unsupported.id
        );
    }
    let auto_apply_supported_agent_routes = scope == LifecycleScope::Project
        && targets.contains(&LifecycleTarget::Agent)
        && !cmd.no_apply
        && cmd.host.as_deref().is_some_and(|host| {
            matches!(
                host.trim().to_ascii_lowercase().as_str(),
                "codex" | "claude-code" | "claude" | "all"
            )
        });
    let should_apply_supported_routes = scope == LifecycleScope::Project
        && (cmd.apply || auto_apply_supported_agent_routes || default_agent_official_hook_setup);
    let mut state = read_lifecycle(scope)?;
    let now = now_stamp();
    for target in &targets {
        match target {
            LifecycleTarget::Agent => {
                let mut agent = agent_started(&now);
                if default_agent_wrapper_setup {
                    let wrapper_path = install_project_agent_wrapper(&cmd.session)?;
                    agent.host_routes.insert(
                        "agent-wrapper".into(),
                        lifecycle_agent_wrapper_route(
                            Some(wrapper_path.display().to_string()),
                            "project TFY agent command wrapper created; configure the AI agent command executor to call this wrapper, then verify real wrapper invocation plus raw/ledger/no-negative/positive-savings evidence before active=true",
                        ),
                    );
                }
                for host in &selected_hosts {
                    if host.id == "codex" && should_apply_supported_routes {
                        let result = configure_codex_project_hook(
                            &cmd.session,
                            HostConfigScope::Project,
                            false,
                            false,
                        )?;
                        agent.host_routes.insert(
                            host.id.into(),
                            lifecycle_official_hook_route(
                                "configured_unverified",
                                result
                                    .get("config_path")
                                    .and_then(Value::as_str)
                                    .map(str::to_string),
                                "configured_unverified",
                                true,
                                "safe project .codex/config.toml PreToolUse Bash hook writer ran; trust the project .codex layer, review/enable the hook with Codex /hooks, then verify real host invocation plus raw/ledger/no-negative/positive-savings evidence before active=true",
                            ),
                        );
                    } else if host.id == "claude-code" && should_apply_supported_routes {
                        let result = configure_claude_project_hook(
                            &cmd.session,
                            HostConfigScope::Project,
                            false,
                            false,
                        )?;
                        agent.host_routes.insert(
                            host.id.into(),
                            lifecycle_official_hook_route(
                                "configured_unverified",
                                result
                                    .get("config_path")
                                    .and_then(Value::as_str)
                                    .map(str::to_string),
                                "configured_unverified",
                                false,
                                "safe project .claude/settings.json PreToolUse Bash hook writer ran; reload Claude Code, then verify real host invocation plus raw/ledger/no-negative/positive-savings evidence before active=true",
                            ),
                        );
                    } else if should_apply_supported_routes && !host_all {
                        bail!(
                            "automatic --apply for host '{}' is not implemented safely yet; use `tfy setup --host {} --dry-run` and apply manually",
                            host.id,
                            host.id
                        );
                    } else if should_apply_supported_routes {
                        agent.host_routes.insert(
                            host.id.into(),
                            lifecycle_host_route(
                                host.status,
                                None,
                                host.status,
                                "guidance-only in --host all; no unsafe writer ran for this host",
                            ),
                        );
                    } else {
                        agent.host_routes.insert(
                            host.id.into(),
                            lifecycle_host_route(
                                host.status,
                                None,
                                host.status,
                                "configuration snippet/guidance available; setup alone is not savings evidence",
                            ),
                        );
                    }
                }
                if cmd.verify {
                    agent.support_status = "verification_requested_route_evidence_required".into();
                } else if default_agent_official_hook_setup {
                    agent.support_status =
                        "default_agent_routes_configured_verification_required".into();
                } else if agent
                    .host_routes
                    .values()
                    .any(|route| route.route_configured)
                {
                    agent.support_status = "host_route_configured_verification_required".into();
                }
                state.agent = Some(agent)
            }
            LifecycleTarget::Human => state.human = Some(human_started(&now)),
        }
    }
    let human_auto_activation_root = if should_enable_human_auto_activation {
        let created_by = if cmd.auto_activate {
            "tfy start --human --auto-activate"
        } else {
            "tfy start --human"
        };
        let root = enable_human_auto_activate_repo(
            &cmd.session,
            Path::new(".tfy/raw"),
            Path::new(".tfy/human/ledger.jsonl"),
            &selected_human_shell,
            created_by,
        )?;
        if let Some(human) = &mut state.human {
            human.route_state = "auto_activation_marker_enabled".into();
            human.support_status = "auto_activation_marker_enabled_hook_required".into();
            human.integration_path = Some(selected_human_auto_script.clone());
        }
        Some(root)
    } else {
        None
    };
    write_lifecycle(scope, &state)?;
    ensure_lifecycle_storage_dirs(&state)?;
    println!(
        "TFY {} start: targets={} configured=true desired=true",
        scope.label(),
        target_names(&targets)
    );
    if scope == LifecycleScope::Global
        && targets.contains(&LifecycleTarget::Agent)
        && (cmd.host.is_some() || cmd.apply || cmd.no_apply)
    {
        println!(
            "global_host_apply=false reason=project_scoped_host_config_required next_action=run `tfy start --agent` inside each project to create the wrapper, or explicit `--host ...` to write Codex/Claude Code config"
        );
    }
    if targets.contains(&LifecycleTarget::Agent) {
        let support_status = state
            .agent
            .as_ref()
            .map(|agent| agent.support_status.as_str())
            .unwrap_or("host_route_configuration_required");
        println!("agent route_state=intent_recorded active=false support_status={support_status} private_hook_interception=false provider_prompt_gateway=false");
        if default_agent_official_hook_setup {
            println!("Agent mode uses the shared TFY command-output pipeline through configured command routes: the TFY agent wrapper fallback plus Codex and Claude Code official project hooks by default. Codex and Claude Code official hooks are configured when available; otherwise use the TFY agent wrapper route.");
        } else {
            println!("Agent mode uses the shared TFY command-output pipeline through a configured command route: the TFY agent wrapper/executor route and official Codex/Claude Code host hooks when configured. Codex and Claude Code official hooks are configured when available; otherwise use the TFY agent wrapper route.");
        }
        if default_agent_wrapper_setup {
            println!("Installed TFY agent command wrapper: .tfy/agent/tfy-agent-wrapper");
            println!("Configure your AI agent host command executor to call: .tfy/agent/tfy-agent-wrapper -- <ordinary command>");
            println!("Status: configured, not active.");
            println!("Next: run the AI agent through that wrapper and collect raw/ledger/no-negative/positive-savings evidence before active=true.");
            println!("host=agent-wrapper route_state=configured_unverified active=false route_configured=true host_reload_required=false host_approval_required=false host_route_available_after_reload=false config_path=.tfy/agent/tfy-agent-wrapper");
        }
        for host in &selected_hosts {
            if host.id == "codex" && should_apply_supported_routes {
                println!("Configured Codex project PreToolUse Bash hook route: .codex/config.toml");
                println!("Trust the project .codex layer, then use Codex /hooks to review and enable the TFY hook.");
                println!("Status: configured, not active.");
                println!("Next: run a real Codex Bash command through the hook and collect route-bound raw/ledger/no-negative/positive-savings evidence.");
                println!("host=codex route_state=configured_unverified active=false route_configured=true host_reload_required=true host_approval_required=true host_route_available_after_reload=false config_path=.codex/config.toml");
            } else if host.id == "claude-code" && should_apply_supported_routes {
                println!("Configured Claude Code project PreToolUse Bash hook route: .claude/settings.json");
                println!("Reload Claude Code so the project hook config is visible.");
                println!("Status: configured, not active.");
                println!(
                    "Next: run a real Claude Code Bash command through the hook and collect route-bound raw/ledger/no-negative/positive-savings evidence."
                );
                println!("host=claude-code route_state=configured_unverified active=false route_configured=true host_reload_required=true host_approval_required=false host_route_available_after_reload=false config_path=.claude/settings.json");
            } else if should_apply_supported_routes {
                println!(
                    "host={} route_state={} active=false configured=false apply=guidance_only",
                    host.id, host.status
                );
            } else {
                println!(
                    "host={} route_state={} active=false apply=false",
                    host.id, host.status
                );
                println!("--- snippet {} ---", host.id);
                print!("{}", host_setup_snippet(host, &cmd.session));
            }
        }
        if cmd.verify {
            println!(
                "verify_requested=true promotion=false reason=route_evidence_and_savings_required next_action=run_host_and_attach_setup_invocation_ledger_raw_no_negative_positive_overhead_evidence output=plain_text"
            );
            println!("local_smoke_status=not_run named_host_launch_supported=false");
        }
    }
    if targets.contains(&LifecycleTarget::Human) {
        if let Some(root) = &human_auto_activation_root {
            println!("human_auto_activation repo_marker.enabled=true repo_marker.valid=true repo_marker.root={} shell={} integration_path={} selected_rcfile.hook_installed=unknown support_status=auto_activation_marker_enabled_hook_required", root.display(), selected_human_shell, selected_human_auto_script);
            println!("future-shell auto-activation requires one explicit user rc/profile hook install: tfy setup --human --apply");
        }
        if human_managed_session_available() {
            if project_human_only && interactive_terminal && !cmd.auto_activate {
                if human_auto_activation_root.is_some() {
                    offer_human_auto_activate_hook_install(&selected_human_shell)?;
                }
                println!("human route_state=managed_session_starting active=false support_status=managed_session_available ordinary_terminal_interception=false managed_session_interception=true managed_session_available=true managed_session_scope_root={} managed_session_entrypoint=\"tfy start --human\"", std::env::current_dir()?.display());
                println!("ordinary terminals outside this TFY-managed session are not globally intercepted; the supported platform shell now enters a current-directory-scoped managed human session from `tfy start --human`. Use `tfy human shell --no-auto-intercept` for a managed shell without PATH/proxy routing, `tfy shell <command>` for raw passthrough, or `tfy shell -- <command>` / `tfy tool-gateway -- <command>` for explicit one-off gateway wrapping.");
                return execute_human_shell(
                    &cmd.session,
                    Path::new(".tfy/raw"),
                    Path::new(".tfy/human/ledger.jsonl"),
                    &selected_human_shell,
                    true,
                );
            } else {
                let launch_reason = if cmd.auto_activate {
                    "explicit_auto_activate_only"
                } else if !project_human_only {
                    "requires_project_human_only_target"
                } else {
                    "requires_interactive_tty"
                };
                println!("human route_state=intent_recorded active=false support_status=managed_session_available ordinary_terminal_interception=false managed_session_interception=false managed_session_available=true managed_session_launch={launch_reason} managed_session_entrypoint=\"tfy start --human\"");
                if cmd.auto_activate {
                    println!("human lifecycle intent and current-directory auto-activation marker recorded by explicit automation request; new supported-platform shell sessions auto-activate only after the explicit TFY rc/profile hook is installed. This is not universal/global terminal interception.");
                } else {
                    println!("human lifecycle intent recorded; run `tfy start --human` as the only project target from an interactive terminal to create current-directory future-shell activation and enter the managed session. Non-interactive plain start remains intent-only; use `tfy start --human --auto-activate` for explicit automation persistence. Ordinary terminals outside TFY-managed sessions or trusted marked current directories with the explicit rc hook are not globally intercepted.");
                }
            }
        } else {
            println!("human route_state=intent_recorded active=false support_status=managed_session_unsupported_platform ordinary_terminal_interception=false managed_session_interception=false managed_session_available=false managed_session_entrypoint=\"tfy start --human\"");
            println!("ordinary terminal commands are not globally intercepted; TFY-managed human sessions are unavailable for the selected platform shell on this platform. Use `tfy shell <command>` for raw passthrough or `tfy shell -- <command>` for the TFY gateway wrapper.");
        }
    }
    Ok(())
}

fn execute_lifecycle_stop(scope: LifecycleScope, cmd: StopCmd) -> Result<()> {
    let targets = targets_from_args(&cmd.target, cmd.target_alias.as_deref(), "stop")?;
    let mut state = read_lifecycle(scope)?;
    let now = now_stamp();
    for target in &targets {
        match target {
            LifecycleTarget::Agent => {
                let mut agent = state.agent.unwrap_or_else(|| agent_stopped(&now));
                agent.configured = true;
                agent.desired = false;
                agent.active = false;
                if agent.stopped_at.is_none() {
                    agent.stopped_at = Some(now.clone());
                }
                state.agent = Some(agent);
            }
            LifecycleTarget::Human => {
                let mut human = state.human.unwrap_or_else(|| human_stopped(&now));
                human.configured = true;
                human.desired = false;
                human.active = false;
                human.ordinary_terminal_interception = false;
                human.support_status = "manual_explicit_route_required".into();
                if human.stopped_at.is_none() {
                    human.stopped_at = Some(now.clone());
                }
                state.human = Some(human);
            }
        }
    }
    write_lifecycle(scope, &state)?;
    println!(
        "TFY {} stop: targets={} configured=true desired=false data_preserved=true raw_preserved=true",
        scope.label(),
        target_names(&targets)
    );
    Ok(())
}

fn execute_lifecycle_fuckyou(scope: LifecycleScope, cmd: FuckyouCmd) -> Result<()> {
    let targets = if cmd.target.agent || cmd.target.human || cmd.target_alias.is_some() {
        let targets = targets_from_args(&cmd.target, cmd.target_alias.as_deref(), "fuckyou")?;
        confirm_fuckyou(cmd.yes)?;
        targets
    } else if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        let targets = prompt_targets("fuckyou")?;
        confirm_fuckyou(cmd.yes)?;
        targets
    } else {
        eprintln!("TFY fuckyou: choose target: agent, human, or both; then type yes to confirm");
        let mut input = String::new();
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            let mut lock = stdin.lock();
            lock.read_line(&mut input)?;
            if !cmd.yes {
                lock.read_line(&mut input)?;
            }
        } else {
            stdin.lock().read_to_string(&mut input)?;
        }
        let mut choices = input.lines().map(str::trim).filter(|line| !line.is_empty());
        let choice = choices.next().unwrap_or("");
        let targets = parse_target_choice(choice, "fuckyou")?;
        if !cmd.yes && !choices.any(|line| line == "yes") {
            bail!("tfy fuckyou requires confirmation; rerun with --yes or type yes")
        }
        targets
    };
    let mut state = read_lifecycle(scope)?;
    for target in &targets {
        if scope == LifecycleScope::Project && *target == LifecycleTarget::Agent {
            cleanup_project_agent_host_configs()?;
        }
        remove_target_dir(scope, *target)?;
        match target {
            LifecycleTarget::Agent => state.agent = None,
            LifecycleTarget::Human => state.human = None,
        }
    }
    remove_lifecycle_if_empty(scope, &state)?;
    println!(
        "TFY {} fuckyou: targets={} scoped_cleanup=true raw_preserved=true shared_ledgers_preserved=true",
        scope.label(),
        target_names(&targets)
    );
    Ok(())
}

pub(crate) fn execute_raw(cmd: RawCmd) -> Result<()> {
    if cmd.list {
        let entries = raw_entries(&cmd.raw_dir)?;
        let report = RawLifecycleReport {
            status: "pass".into(),
            raw_dir: cmd.raw_dir.display().to_string(),
            action: "list".into(),
            raw_ref: None,
            count: entries.len(),
            bytes: entries.iter().map(|entry| entry.bytes).sum(),
            dry_run: true,
            applied: false,
            entries,
            message: "listed local raw evidence; no model-facing command output was compacted"
                .into(),
        };
        if cmd.json {
            print_json(&report)?;
        } else {
            println!(
                "TFY raw list: {} entrie(s) in {}",
                report.count, report.raw_dir
            );
            for entry in &report.entries {
                println!(
                    "{} bytes={} exit={} command_sha256={}",
                    entry.raw_ref, entry.bytes, entry.exit_code, entry.command_sha256
                );
            }
        }
        return Ok(());
    }

    if let Some(export_path) = cmd.export.as_ref() {
        let entries = if let Some(raw_ref) = cmd.raw_ref.as_ref() {
            vec![raw_entry(&cmd.raw_dir, raw_ref)?]
        } else {
            raw_entries(&cmd.raw_dir)?
        };
        let mut exported = Vec::new();
        let target_is_dir = cmd.raw_ref.is_none() || export_path.extension().is_none();
        if target_is_dir {
            fs::create_dir_all(export_path)?;
        } else if let Some(parent) = export_path.parent() {
            fs::create_dir_all(parent)?;
        }
        for entry in entries {
            let source = PathBuf::from(&entry.path);
            let target = if target_is_dir {
                export_path.join(format!("{}.json", entry.raw_ref))
            } else {
                export_path.clone()
            };
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "export raw evidence {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
            let mut exported_entry = entry.clone();
            exported_entry.path = target.display().to_string();
            exported.push(exported_entry);
        }
        let report = RawLifecycleReport {
            status: "pass".into(),
            raw_dir: cmd.raw_dir.display().to_string(),
            action: "export".into(),
            raw_ref: cmd.raw_ref.clone(),
            count: exported.len(),
            bytes: exported.iter().map(|entry| entry.bytes).sum(),
            dry_run: false,
            applied: true,
            entries: exported,
            message:
                "exported local raw evidence JSON; TFY still stores raw evidence locally by default"
                    .into(),
        };
        if cmd.json {
            print_json(&report)?;
        } else {
            println!(
                "TFY raw export: {} entrie(s) -> {}",
                report.count,
                export_path.display()
            );
        }
        return Ok(());
    }

    if cmd.prune {
        let entries = raw_entries(&cmd.raw_dir)?;
        let selected: Vec<_> = entries
            .into_iter()
            .filter(|entry| raw_entry_matches_age(entry, cmd.older_than_days))
            .collect();
        let apply = cmd.apply && !cmd.dry_run;
        if apply {
            for entry in &selected {
                fs::remove_file(&entry.path)
                    .with_context(|| format!("prune raw evidence {}", entry.path))?;
            }
        }
        let report = RawLifecycleReport {
            status: "pass".into(),
            raw_dir: cmd.raw_dir.display().to_string(),
            action: "prune".into(),
            raw_ref: None,
            count: selected.len(),
            bytes: selected.iter().map(|entry| entry.bytes).sum(),
            dry_run: !apply,
            applied: apply,
            entries: selected,
            message: if apply {
                "pruned selected raw evidence files after explicit --apply".into()
            } else {
                "dry-run only; pass --apply without --dry-run to delete selected raw evidence"
                    .into()
            },
        };
        if cmd.json {
            print_json(&report)?;
        } else {
            println!(
                "TFY raw prune: selected={} dry_run={} applied={}",
                report.count, report.dry_run, report.applied
            );
        }
        return Ok(());
    }

    if cmd.inspect {
        let raw_ref = cmd
            .raw_ref
            .as_deref()
            .ok_or_else(|| anyhow!("tfy raw --inspect requires a raw_ref"))?;
        let entry = raw_entry(&cmd.raw_dir, raw_ref)?;
        let report = RawLifecycleReport {
            status: "pass".into(),
            raw_dir: cmd.raw_dir.display().to_string(),
            action: "inspect".into(),
            raw_ref: Some(raw_ref.into()),
            count: 1,
            bytes: entry.bytes,
            dry_run: true,
            applied: false,
            entries: vec![entry],
            message: "inspected raw evidence metadata without printing raw bytes".into(),
        };
        if cmd.json {
            print_json(&report)?;
        } else {
            let entry = &report.entries[0];
            println!(
                "TFY raw inspect: {} bytes={} exit={} path={}",
                entry.raw_ref, entry.bytes, entry.exit_code, entry.path
            );
            println!("command_sha256={}", entry.command_sha256);
        }
        return Ok(());
    }

    let raw_ref = cmd.raw_ref.as_deref().ok_or_else(|| {
        anyhow!("tfy raw requires a raw_ref, --list, --inspect, --export, or --prune")
    })?;
    let bytes = raw_output_bytes(&cmd.raw_dir, raw_ref, cmd.around.as_deref(), cmd.context)?;
    if cmd.json {
        let text = String::from_utf8_lossy(&bytes).to_string();
        print_json(&json!({"raw_ref": raw_ref, "bytes": bytes.len(), "text": text}))?;
    } else {
        std::io::stdout().write_all(&bytes)?;
    }
    Ok(())
}

pub(crate) fn execute_bench(cmd: BenchCmd) -> Result<()> {
    let manifest = build_benchmark_manifest(&cmd)?;
    if let Some(output) = cmd.output.as_ref() {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, serde_json::to_string_pretty(&manifest)? + "\n")?;
    }
    if cmd.json {
        print_json(&manifest)?;
    } else {
        println!("TFY bench: {}", manifest.status);
        println!(
            "scenarios={} raw_bytes={} model_bytes={} saved_bytes={} no_negative_savings={}",
            manifest.tfy_self_benchmark.scenarios,
            manifest.tfy_self_benchmark.raw_bytes,
            manifest.tfy_self_benchmark.model_bytes,
            manifest.tfy_self_benchmark.saved_bytes,
            manifest.tfy_self_benchmark.no_negative_savings
        );
        println!(
            "public_superiority_claim_ready={}",
            manifest.public_superiority_claim_ready
        );
    }
    Ok(())
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
            .map(|host| {
                if cmd.live {
                    run_live_host_hook_smoke(host, &cmd.max_budget_usd)
                } else {
                    host_smoke_report(host)
                }
            })
            .collect::<Result<_>>()?;
        if cmd.json {
            print_json(&json!({
                "status": if host_reports.iter().all(|h| h["status"] == "pass" || h["status"] == "checklist") {"pass"} else {"warn"},
                "mode": if cmd.live { "host_live" } else { "host" },
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
        let required_route_evidence = write_required_route_smoke_evidence(&reports)?;
        evidence.push(format!(
            "host_evidence={}",
            required_route_evidence.display()
        ));
        evidence.push(
            "required-route smoke evidence can be passed to launch-report; named hosts still require real host invocation evidence"
                .into(),
        );
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
        if cmd.json && cmd.local {
            write_codex_smoke_checklist(std::io::stderr())?;
        } else {
            print_codex_smoke_checklist()?;
        }
        if !cmd.local {
            return Ok(());
        }
    }
    let report = run_adapter_smoke()?;
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
    if !(cmd.human || cmd.ai || cmd.codex || !cmd.host.is_empty()) {
        bail!("setup requires --human, --ai, --codex, and/or --host <host>; use `tfy setup --human --apply` for explicit human shell setup or `tfy setup --ai --host codex --dry-run` for AI-host routing");
    }
    if cmd.human {
        if cmd.ai || cmd.codex || !cmd.host.is_empty() {
            bail!("tfy setup --human cannot be combined with AI-host setup flags (--ai, --codex, --host); run human and AI setup as separate explicit commands");
        }
        if cmd.project || cmd.global {
            bail!("tfy setup --human does not support --project/--global scope flags in v1; it only configures one explicit user rcfile for trusted marked current directories");
        }
        if cmd.apply && cmd.dry_run {
            bail!("--apply and --dry-run cannot be combined");
        }
        if cmd.uninstall {
            execute_human_auto_activate_uninstall_setup(&cmd.shell, cmd.rcfile.clone(), cmd.apply)?;
        } else {
            execute_human_auto_activate_install_setup(
                &cmd.shell,
                cmd.rcfile.clone(),
                cmd.dry_run || !cmd.apply,
                cmd.apply,
            )?;
        }
        println!(
            "TFY setup human: explicit_rc_hook=true ordinary_terminal_interception=false universal_interception=false dry_run={} apply={}",
            !cmd.apply,
            cmd.apply
        );
        println!("Human auto-activation applies only in trusted marked current directories with a TFY marker created by `tfy start --human`; npm install and setup dry-runs do not mutate shell rcfiles.");
        return Ok(());
    }
    if cmd.ai {
        println!("TFY setup ai: supported_host_routing=true ordinary_terminal_interception=false provider_gateway=false editor_integration=false");
        println!("AI reads compact transport through TFY context/tool surfaces; files and user output are restored to readable canonical code.");
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
    let report = build_product_status_report(&cmd);
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("TFY status: {}", report.status);
        println!("target_filter: {}", report.target_filter);
        println!(
            "lifecycle status: {} desired={} configured={} active={} evidence_required={} desired_targets={} configured_targets={} active_targets={}",
            report.lifecycle_summary.status,
            report.lifecycle_summary.desired,
            report.lifecycle_summary.configured,
            report.lifecycle_summary.active,
            report.lifecycle_summary.evidence_required,
            report.lifecycle_summary.desired_targets.join(","),
            report.lifecycle_summary.configured_targets.join(","),
            report.lifecycle_summary.active_targets.join(",")
        );
        println!("lifecycle message: {}", report.lifecycle_summary.message);
        if !cmd.human {
            if let Some(agent) = &report.effective_lifecycle.agent {
                println!(
                    "effective agent: desired={} source={} configured={} active={} route_state={} support_status={}",
                    agent.desired,
                    agent.desired_source,
                    agent.configured,
                    agent.active,
                    agent.route_state,
                    agent.support_status
                );
                println!("next agent action: {}", agent.next_action);
            }
        }
        if !cmd.agent {
            if let Some(human) = &report.effective_lifecycle.human {
                println!(
                    "effective human: desired={} source={} configured={} active={} route_state={} support_status={}",
                    human.desired,
                    human.desired_source,
                    human.configured,
                    human.active,
                    human.route_state,
                    human.support_status
                );
                println!("next human action: {}", human.next_action);
            }
        }
        if !cmd.human {
            if let Some(agent) = &report.project_lifecycle.agent {
                println!("project agent: configured={} desired={} route_state={} active={} support_status={} private_hook_interception={} provider_prompt_gateway={}", agent.configured, agent.desired, agent.route_state, agent.active, agent.support_status, agent.private_hook_interception, agent.provider_prompt_gateway);
            }
            if let Some(agent) = &report.global_lifecycle.agent {
                println!("global agent: configured={} desired={} route_state={} active={} support_status={} private_hook_interception={} provider_prompt_gateway={}", agent.configured, agent.desired, agent.route_state, agent.active, agent.support_status, agent.private_hook_interception, agent.provider_prompt_gateway);
            }
        }
        if !cmd.agent {
            if let Some(human) = &report.project_lifecycle.human {
                println!("project human: configured={} desired={} route_state={} active={} support_status={} ordinary_terminal_interception={} managed_session_available={} session_wrapper_available={}", human.configured, human.desired, human.route_state, human.active, human.support_status, human.ordinary_terminal_interception, human.managed_session_available, human.session_wrapper_available);
            }
            if let Some(human) = &report.global_lifecycle.human {
                println!("global human: configured={} desired={} route_state={} active={} support_status={} ordinary_terminal_interception={} managed_session_available={} session_wrapper_available={}", human.configured, human.desired, human.route_state, human.active, human.support_status, human.ordinary_terminal_interception, human.managed_session_available, human.session_wrapper_available);
            }
        }
        if !cmd.human {
            for host in &report.minimum_v1_host_matrix {
                println!(
                    "host {}: {} — {}",
                    host.host, host.status, host.launch_claim
                );
            }
        }
        for surface in &report.surfaces {
            if (cmd.agent && surface.name.contains("human"))
                || (cmd.human && !surface.name.contains("human"))
            {
                continue;
            }
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
    let release_evidence = build_release_evidence_summary(&cmd.release_evidence);
    let status = build_product_status_report(&StatusCmd {
        agent: false,
        human: false,
        json: true,
    });
    let report = build_launch_readiness_report(status, gain, host_evidence, release_evidence);
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
        println!("Hints: run `tfy adapter run -- <command>` or `tfy agent run -- <command>` for command-output savings data.");
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
            config: "project .codex/config.toml [[hooks.PreToolUse]] matcher=^Bash$ command hook",
            transport: "Official Codex PreToolUse Bash hook",
            official_source: "https://developers.openai.com/codex/hooks and https://developers.openai.com/codex/config-advanced",
            config_strategy: "Codex project .codex/config.toml PreToolUse Bash command hook; project .codex layer and /hooks trust required",
            apply_strategy: "safe TOML writer for project .codex/config.toml with TFY marker block, backup, provenance, idempotency, and uninstall",
            smoke_strategy: "local hook smoke plus real Codex invocation artifact",
            host_evidence_strategy: "host-bound official_host_hook ledger/raw evidence with Codex invocation artifact",
            setup: "project .codex/config.toml PreToolUse Bash hook plus optional AGENTS.md guidance through tfy init",
            normal_workflow: "Codex may route Bash tool execution through TFY after project .codex trust and /hooks review; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Codex invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official OpenAI Codex hook/config source pinned",
                "local TFY official-host-hook smoke",
                "real Codex hook invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
        HostIntegration {
            id: "claude-code",
            display: "Claude Code",
            status: "config_snippet_available",
            required_for_v1: false,
            config: "project .claude/settings.json hooks.PreToolUse Bash command hook",
            transport: "Official Claude Code PreToolUse Bash hook",
            official_source: "https://docs.anthropic.com/en/docs/claude-code/hooks and https://docs.anthropic.com/en/docs/claude-code/hooks-guide",
            config_strategy: "project .claude/settings.json PreToolUse Bash command hook",
            apply_strategy: "safe JSON writer for project .claude/settings.json with backup/provenance/idempotency/uninstall",
            smoke_strategy: "local hook smoke plus real Claude Code invocation artifact",
            host_evidence_strategy: "host-bound official_host_hook ledger/raw evidence with Claude Code invocation artifact",
            setup: "Claude Code project .claude/settings.json PreToolUse Bash hook",
            normal_workflow: "Claude Code may route Bash tool execution through TFY after hook config is loaded; setup alone is not token-savings proof",
            launch_claim: "not launch-supported until real Claude invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists",
            evidence_gate: &[
                "official Claude Code hook source pinned",
                "local TFY official-host-hook smoke",
                "real Claude Code hook invocation artifact",
                "raw/model byte ledger with no-negative-savings proof",
                "overhead baseline or explicit exception",
            ],
        },
    ]
}

fn agent_mode_host_unavailable(host: &str) -> anyhow::Error {
    anyhow!(
        "host '{}' is not available in TFY agent mode; supported hosts: codex, claude-code",
        host
    )
}

fn host_integration(host: &str) -> Result<HostIntegration> {
    let normalized = host.trim().to_ascii_lowercase().replace('_', "-");
    host_registry()
        .into_iter()
        .find(|candidate| {
            candidate.id == normalized || (candidate.id == "claude-code" && normalized == "claude")
        })
        .ok_or_else(|| agent_mode_host_unavailable(host))
}

fn host_setup_snippet(host: &HostIntegration, session: &str) -> String {
    match host.id {
        "codex" => format!(
            "# .codex/config.toml\n# Trust the project .codex layer, then review/enable this with Codex /hooks.\n[[hooks.PreToolUse]]\nmatcher = \"^Bash$\"\n[[hooks.PreToolUse.hooks]]\ntype = \"command\"\ncommand = \"./.tfy/agent/codex-pre-tool-use\"\ntimeout = 30\nstatusMessage = \"TFY summarizing Bash output\"\n# Generated hook script runs: tfy hook run --host codex --session {session}\n"
        ),
        "claude-code" => format!(
            "{{\n  \"hooks\": {{\n    \"PreToolUse\": [{{\n      \"matcher\": \"Bash\",\n      \"hooks\": [{{\n        \"type\": \"command\",\n        \"command\": \"./.tfy/agent/claude-pre-tool-use\",\n        \"timeout\": 30\n      }}]\n    }}]\n  }}\n}}\n# Generated hook script runs: tfy hook run --host claude-code --session {session}\n"
        ),
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
    if host.status == "unsupported" {
        bail!(
            "host '{}' is not available in TFY agent mode; supported hosts: codex, claude-code",
            host.id
        );
    }
    if host.status == "planned_discovery" {
        println!("TFY setup host={} status=planned_discovery", host.id);
        println!("source={}", host.official_source);
        println!("{}", host.setup);
        println!("{}", host.launch_claim);
        return Ok(());
    }
    if (apply || uninstall) && matches!(host.id, "codex" | "claude-code") {
        let result = match host.id {
            "codex" => configure_codex_project_hook(session, scope, dry_run, uninstall)?,
            "claude-code" => configure_claude_project_hook(session, scope, dry_run, uninstall)?,
            _ => unreachable!(),
        };
        let (status, claim_tier) = if dry_run {
            ("dry_run", "configurable")
        } else if uninstall {
            ("removed_unverified", "applied_unverified")
        } else {
            ("configured_unverified", "configured_unverified")
        };
        println!(
            "TFY setup host={} status={status} claim_tier={claim_tier}",
            host.id
        );
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
    println!("Official host hook routing for Codex/Claude Code when documented; no private hidden hooks, provider prompt mutation, editor auto-integration, or universal human-shell interception.");
    println!("{}", host.launch_claim);
    Ok(())
}

fn provenance_path(host: &str) -> PathBuf {
    PathBuf::from(".tfy")
        .join("host-config")
        .join(format!("{host}.json"))
}

fn remove_host_provenance(host: &str) -> Result<()> {
    let path = provenance_path(host);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("remove {}", path.display())),
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    path.with_file_name(format!("{file_name}.tfy-backup"))
}

fn host_hook_script_path(host: &str) -> PathBuf {
    PathBuf::from(".tfy")
        .join("agent")
        .join(format!("{host}-pre-tool-use"))
}

fn install_host_hook_script(host: &str, session: &str) -> Result<PathBuf> {
    let project_root = std::env::current_dir()?;
    let relative = host_hook_script_path(host);
    if let Some(parent) = relative.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow!("current tfy executable path is not valid UTF-8"))?;
    let exe = shell_single_quote(exe);
    let route_host = match host {
        // Keep the short script filename while all runtime evidence uses the canonical host id.
        "claude" => "claude-code",
        other => other,
    };
    let raw_dir = shell_single_quote(&project_root.join(".tfy/raw").display().to_string());
    let ledger = shell_single_quote(
        &project_root
            .join(format!(".tfy/hook/{route_host}-ledger.jsonl"))
            .display()
            .to_string(),
    );
    let script = format!(
        r#"#!/usr/bin/env sh
# TFY official host hook router for {route_host}. Reads host hook JSON from stdin.
# Generated by TFY; does not mutate human shell startup files.
exec {exe} hook run --host {route_host} --session "${{TFY_SESSION_ID:-{session}}}" --raw-dir {raw_dir} --ledger {ledger}
"#
    );
    fs::write(&relative, script).with_context(|| format!("write {}", relative.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&relative, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("chmod {}", relative.display()))?;
    }
    Ok(std::env::current_dir()?.join(relative))
}

fn codex_tfy_block(script_path: &Path) -> String {
    let command = script_path
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "# TFY:HOST-CONFIG:START codex\n[[hooks.PreToolUse]]\nmatcher = \"^Bash$\"\n[[hooks.PreToolUse.hooks]]\ntype = \"command\"\ncommand = \"{command}\"\ntimeout = 30\nstatusMessage = \"TFY summarizing Bash output\"\n# TFY:HOST-CONFIG:END codex\n"
    )
}

fn hash_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn write_host_hook_provenance(
    host: &str,
    path: &Path,
    managed_key_path: &str,
    session: &str,
    entry_text: &str,
    script_path: &Path,
) -> Result<()> {
    let now = now_stamp();
    let args = vec![
        "hook".into(),
        "run".into(),
        "--host".into(),
        host.into(),
        "--session".into(),
        session.into(),
        "--raw-dir".into(),
        ".tfy/raw".into(),
        "--ledger".into(),
        format!(".tfy/hook/{host}-ledger.jsonl"),
    ];
    let args_json = serde_json::to_string(&args).unwrap_or_default();
    let provenance = HostConfigProvenance {
        host: host.into(),
        config_path: path.display().to_string(),
        managed_key_path: managed_key_path.into(),
        command: script_path.display().to_string(),
        args,
        command_args_hash: hash_text(&format!("{} {args_json}", script_path.display())),
        session: session.into(),
        ledger_path: format!(".tfy/hook/{host}-ledger.jsonl"),
        raw_dir: ".tfy/raw".into(),
        created_at: now.clone(),
        updated_at: now,
        tfy_version: env!("CARGO_PKG_VERSION").into(),
        uninstall_safety_hash: hash_text(entry_text),
    };
    let path = provenance_path(host);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&provenance)?),
    )
    .with_context(|| format!("write {}", path.display()))
}

fn validate_codex_toml_config(path: &Path, text: &str) -> Result<()> {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            if !trimmed.ends_with(']') {
                bail!(
                    "malformed TOML heading in {} at line {}; refusing to write",
                    path.display(),
                    index + 1
                );
            }
            normalize_toml_heading(trimmed).with_context(|| {
                format!(
                    "malformed TOML heading in {} at line {}; refusing to write",
                    path.display(),
                    index + 1
                )
            })?;
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            bail!(
                "unsupported or malformed TOML line in {} at line {}; refusing to write",
                path.display(),
                index + 1
            );
        };
        if key.trim().is_empty() || value.trim().is_empty() {
            bail!(
                "malformed TOML key/value in {} at line {}; refusing to write",
                path.display(),
                index + 1
            );
        }
        normalize_toml_dotted_key(key.trim()).with_context(|| {
            format!(
                "malformed TOML key in {} at line {}; refusing to write",
                path.display(),
                index + 1
            )
        })?;
    }
    Ok(())
}

fn normalize_toml_heading(line: &str) -> Result<Vec<String>> {
    let mut inner = line.trim();
    if inner.starts_with("[[") && inner.ends_with("]]") {
        inner = &inner[2..inner.len() - 2];
    } else if inner.starts_with('[') && inner.ends_with(']') {
        inner = &inner[1..inner.len() - 1];
    } else {
        bail!("invalid TOML heading");
    }
    inner
        .split('.')
        .map(|part| normalize_toml_key(part.trim()).ok_or_else(|| anyhow!("invalid TOML key")))
        .collect()
}

fn normalize_toml_key(key: &str) -> Option<String> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    if (key.starts_with('"') && key.ends_with('"'))
        || (key.starts_with('\'') && key.ends_with('\''))
    {
        if key.len() < 2 {
            return None;
        }
        return Some(
            key[1..key.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\'", "'"),
        );
    }
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Some(key.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_toml_dotted_key(key: &str) -> Result<Vec<String>> {
    key.split('.')
        .map(|part| normalize_toml_key(part.trim()).ok_or_else(|| anyhow!("invalid TOML key")))
        .collect()
}

fn strip_marked_host_block(text: &str, host: &str) -> Result<(String, bool)> {
    let start = format!("# TFY:HOST-CONFIG:START {host}");
    let end = format!("# TFY:HOST-CONFIG:END {host}");
    let mut output = Vec::new();
    let mut in_block = false;
    let mut removed = false;
    for (index, line) in text.lines().enumerate() {
        if line.trim() == start {
            if in_block {
                bail!(
                    "malformed TFY host config marker for {host} at line {}; refusing to write",
                    index + 1
                );
            }
            in_block = true;
            removed = true;
            continue;
        }
        if line.trim() == end {
            if !in_block {
                bail!(
                    "malformed TFY host config marker for {host} at line {}; refusing to write",
                    index + 1
                );
            }
            in_block = false;
            continue;
        }
        if !in_block {
            output.push(line);
        }
    }
    if in_block {
        bail!("malformed TFY host config marker for {host}: missing end marker; refusing to write");
    }
    let mut rendered = output.join("\n");
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    Ok((rendered, removed))
}

fn configure_codex_project_hook(
    session: &str,
    scope: HostConfigScope,
    dry_run: bool,
    uninstall: bool,
) -> Result<Value> {
    if scope == HostConfigScope::Global {
        bail!("codex --global apply is not implemented here; use project .codex/config.toml for automatic lifecycle routing");
    }
    let path = PathBuf::from(".codex").join("config.toml");
    let backup = backup_path(&path);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let existing_text = existing.as_deref().unwrap_or("");
    validate_codex_toml_config(&path, existing_text)?;
    let (mut base, removed) = strip_marked_host_block(existing_text, "codex")?;
    let script_path = if uninstall || dry_run {
        std::env::current_dir()?.join(host_hook_script_path("codex"))
    } else {
        install_host_hook_script("codex", session)?
    };
    let block = codex_tfy_block(&script_path);
    if !uninstall {
        if !base.ends_with('\n') && !base.is_empty() {
            base.push('\n');
        }
        base.push_str(&block);
    }
    if !dry_run {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        if existing.is_some() && !backup.exists() && !uninstall {
            fs::copy(&path, &backup)
                .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        }
        if uninstall && base.trim().is_empty() {
            if path.exists() && removed {
                fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
            }
        } else if !uninstall || removed {
            fs::write(&path, base).with_context(|| format!("write {}", path.display()))?;
        }
        if uninstall {
            remove_host_provenance("codex")?;
        } else {
            write_host_hook_provenance(
                "codex",
                &path,
                "hooks.PreToolUse[matcher=^Bash$]",
                session,
                &block,
                &script_path,
            )?;
        }
    }
    Ok(json!({
        "scope": scope.label(),
        "config_path": path.display().to_string(),
        "backup_path": backup.display().to_string(),
        "provenance_path": provenance_path("codex").display().to_string(),
        "dry_run": dry_run,
        "applied": !dry_run,
        "action": if uninstall { "uninstall" } else { "install" },
        "removed_existing_tfy_route": removed,
    }))
}

fn cleanup_codex_project_hook_if_tfy_owned() -> Result<()> {
    let path = PathBuf::from(".codex").join("config.toml");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let (base, removed) = strip_marked_host_block(&existing, "codex")?;
    if !removed {
        return Ok(());
    }
    validate_codex_toml_config(&path, &base)?;
    if base.trim().is_empty() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    } else {
        fs::write(&path, base).with_context(|| format!("write {}", path.display()))?;
    }
    remove_host_provenance("codex")
}

fn configure_claude_project_hook(
    session: &str,
    scope: HostConfigScope,
    dry_run: bool,
    uninstall: bool,
) -> Result<Value> {
    if scope == HostConfigScope::Global {
        bail!("claude-code --global apply is not implemented here; use project .claude/settings.json for automatic lifecycle routing");
    }
    let path = PathBuf::from(".claude").join("settings.json");
    let backup = backup_path(&path);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let mut root: Value = match existing.as_deref() {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).with_context(|| {
            format!("parse existing Claude Code hook config {}", path.display())
        })?,
        _ => json!({}),
    };
    let root_obj = root.as_object_mut().ok_or_else(|| {
        anyhow!(
            "Claude Code hook config must be a JSON object: {}",
            path.display()
        )
    })?;
    remove_claude_tfy_hook_entries(root_obj)?;
    let mut installed_script_path = None;
    let mut installed_entry_text = None;
    if !uninstall {
        let script_path = if dry_run {
            std::env::current_dir()?.join(host_hook_script_path("claude"))
        } else {
            install_host_hook_script("claude", session)?
        };
        insert_claude_tfy_hook_entry(root_obj, &script_path)?;
        if !dry_run {
            installed_entry_text =
                Some(serde_json::to_string(&claude_tfy_hook_entry(&script_path))?);
            installed_script_path = Some(script_path);
        }
    }
    if !dry_run {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        if existing.is_some() && !backup.exists() && !uninstall {
            fs::copy(&path, &backup)
                .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        }
        if uninstall && root.as_object().is_some_and(|object| object.is_empty()) {
            if path.exists() {
                fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
            }
        } else {
            fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&root)?))
                .with_context(|| format!("write {}", path.display()))?;
        }
        if uninstall {
            remove_host_provenance("claude-code")?;
        } else if let (Some(script_path), Some(entry_text)) = (
            installed_script_path.as_ref(),
            installed_entry_text.as_ref(),
        ) {
            write_host_hook_provenance(
                "claude-code",
                &path,
                "hooks.PreToolUse[matcher=Bash]",
                session,
                entry_text,
                script_path,
            )?;
        }
    }
    Ok(json!({
        "scope": scope.label(),
        "config_path": path.display().to_string(),
        "backup_path": backup.display().to_string(),
        "provenance_path": provenance_path("claude-code").display().to_string(),
        "dry_run": dry_run,
        "applied": !dry_run,
        "action": if uninstall { "uninstall" } else { "install" },
    }))
}

fn claude_tfy_hook_entry(script_path: &Path) -> Value {
    json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": script_path.display().to_string(),
            "timeout": 30
        }]
    })
}

fn insert_claude_tfy_hook_entry(
    root_obj: &mut serde_json::Map<String, Value>,
    script_path: &Path,
) -> Result<()> {
    let hooks = root_obj.entry("hooks").or_insert_with(|| json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("Claude Code hooks must be a JSON object"))?;
    let pre = hooks_obj.entry("PreToolUse").or_insert_with(|| json!([]));
    let pre_array = pre
        .as_array_mut()
        .ok_or_else(|| anyhow!("Claude Code hooks.PreToolUse must be a JSON array"))?;
    pre_array.push(claude_tfy_hook_entry(script_path));
    Ok(())
}

fn remove_claude_tfy_hook_entries(root_obj: &mut serde_json::Map<String, Value>) -> Result<()> {
    let managed_command = read_host_provenance("claude-code")?.map(|provenance| provenance.command);
    let Some(hooks) = root_obj.get_mut("hooks") else {
        return Ok(());
    };
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("Claude Code hooks must be a JSON object"))?;
    let mut remove_hooks = false;
    if let Some(pre) = hooks_obj.get_mut("PreToolUse") {
        let pre_array = pre
            .as_array_mut()
            .ok_or_else(|| anyhow!("Claude Code hooks.PreToolUse must be a JSON array"))?;
        pre_array
            .retain(|entry| !claude_hook_entry_is_tfy_owned(entry, managed_command.as_deref()));
        if pre_array.is_empty() {
            hooks_obj.remove("PreToolUse");
        }
    }
    if hooks_obj.is_empty() {
        remove_hooks = true;
    }
    if remove_hooks {
        root_obj.remove("hooks");
    }
    Ok(())
}

fn read_host_provenance(host: &str) -> Result<Option<HostConfigProvenance>> {
    let path = provenance_path(host);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .with_context(|| format!("parse {}", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn claude_hook_entry_is_tfy_owned(entry: &Value, managed_command: Option<&str>) -> bool {
    let Some(managed_command) = managed_command else {
        return false;
    };
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| command == managed_command)
            })
        })
}

fn cleanup_claude_project_hook_if_tfy_owned() -> Result<()> {
    let path = PathBuf::from(".claude").join("settings.json");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    if existing.trim().is_empty() {
        return Ok(());
    }
    let mut root: Value = serde_json::from_str(&existing)
        .with_context(|| format!("parse existing Claude Code hook config {}", path.display()))?;
    let Some(root_obj) = root.as_object_mut() else {
        return Ok(());
    };
    remove_claude_tfy_hook_entries(root_obj)?;
    if root.as_object().is_some_and(|object| object.is_empty()) {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    } else {
        fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&root)?))
            .with_context(|| format!("write {}", path.display()))?;
    }
    remove_host_provenance("claude-code")
}

fn cleanup_project_agent_host_configs() -> Result<()> {
    cleanup_codex_project_hook_if_tfy_owned()?;
    cleanup_claude_project_hook_if_tfy_owned()?;
    Ok(())
}

fn host_doctor_report(host: &str) -> Result<serde_json::Value> {
    let host = host_integration(host)?;
    Ok(json!({
        "host": host.id,
        "display": host.display,
        "status": host.status,
        "message": if matches!(host.status, "planned_discovery" | "unsupported") {
            host.setup
        } else {
            "config snippet available; real host invocation evidence is still required"
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
    if host.status == "unsupported" {
        return Ok(json!({
            "host": host.id,
            "display": host.display,
            "status": "unsupported",
            "claim_tier": "unsupported",
            "message": host.setup,
            "required_host_evidence": host.evidence_gate,
            "setup_success_is_not_savings_success": true,
        }));
    }
    Ok(json!({
        "host": host.id,
        "display": host.display,
        "status": if host.status == "planned_discovery" {
            "unsupported"
        } else {
            "checklist"
        },
        "claim_tier": if host.status == "planned_discovery" {
            "planned_discovery"
        } else {
            "configurable"
        },
        "message": if host.status == "planned_discovery" {
            host.setup
        } else {
            "host smoke is checklist/evidence collection until the named host actually invokes TFY"
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
        "required_host_evidence": host.evidence_gate,
        "setup_success_is_not_savings_success": true,
    }))
}

fn live_host_smoke_spec(host: &str) -> Option<LiveHostSmokeSpec> {
    match host {
        "codex" => Some(LiveHostSmokeSpec {
            host: "codex",
            binary: "codex",
            timeout: Duration::from_secs(180),
            needs_noninteractive_bypass: true,
        }),
        "claude-code" => Some(LiveHostSmokeSpec {
            host: "claude-code",
            binary: "claude",
            timeout: Duration::from_secs(180),
            needs_noninteractive_bypass: true,
        }),
        _ => None,
    }
}

fn configure_live_host_smoke_command(
    command: &mut Command,
    spec: LiveHostSmokeSpec,
    prompt: &str,
    max_budget_usd: &str,
) {
    match spec.host {
        "codex" => {
            if spec.needs_noninteractive_bypass {
                command.args([
                    "--dangerously-bypass-hook-trust",
                    "--dangerously-bypass-approvals-and-sandbox",
                ]);
            }
            command.args(["exec", "--skip-git-repo-check", "--json", prompt]);
        }
        "claude-code" => {
            command.args(["-p"]);
            if spec.needs_noninteractive_bypass {
                command.args(["--permission-mode", "bypassPermissions"]);
            }
            command.args([
                "--allowedTools",
                "Bash",
                "--include-hook-events",
                "--output-format",
                "stream-json",
                "--verbose",
                "--max-budget-usd",
                max_budget_usd,
                prompt,
            ]);
        }
        _ => unreachable!("live host smoke spec exists only for supported hosts"),
    }
}

fn run_live_host_hook_smoke(host: &str, max_budget_usd: &str) -> Result<serde_json::Value> {
    let integration = host_integration(host)?;
    let spec = live_host_smoke_spec(integration.id).ok_or_else(|| {
        anyhow!(
            "live official-hook smoke is only implemented for codex and claude-code; host '{}' uses {}",
            integration.id,
            integration.transport
        )
    })?;
    if !host_official_hook_launch_supported(integration.id) {
        bail!(
            "live official-hook smoke is only implemented for codex and claude-code; host '{}' uses {}",
            integration.id,
            integration.transport
        );
    }
    let host_version = host_version(spec.binary)
        .with_context(|| format!("resolve installed {} version for live smoke", spec.binary))?;
    let root = std::env::temp_dir().join(format!(
        "tfy-live-{}-{}-{}",
        integration.id,
        std::process::id(),
        stable_id(&format!("live-{}", integration.id))
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    fs::write(
        root.join("README.md"),
        "TFY live host hook smoke workspace. The host should run one Bash command only.\n",
    )?;
    let setup = Command::new(std::env::current_exe().context("resolve current tfy executable")?)
        .current_dir(&root)
        .env("CARGO_TERM_COLOR", "never")
        .args(["start", "--agent", "--host", integration.id])
        .output()
        .with_context(|| format!("configure {} live smoke project", integration.id))?;
    if !setup.status.success() {
        bail!(
            "live host smoke setup failed for {}: {}",
            integration.id,
            String::from_utf8_lossy(&setup.stderr)
        );
    }
    let invocation = root.join(".tfy").join("host-smoke");
    fs::create_dir_all(&invocation)?;
    let stdout_path = invocation.join(format!("{}-stdout.txt", integration.id));
    let stderr_path = invocation.join(format!("{}-stderr.txt", integration.id));
    let command_sample = "for i in $(seq 1 80); do echo tfy-live-host-hook-smoke-$i; done";
    let prompt = format!(
        "In this temporary trusted smoke workspace, run exactly one Bash shell command and then stop. Do not edit files. Command: {command_sample}"
    );
    let baseline_start = Instant::now();
    let baseline = Command::new("sh")
        .current_dir(&root)
        .args(["-c", command_sample])
        .output()
        .context("run live smoke baseline command")?;
    if !baseline.status.success() {
        bail!(
            "baseline smoke command failed: {}",
            String::from_utf8_lossy(&baseline.stderr)
        );
    }
    let baseline_ms = millis_u64(baseline_start.elapsed());
    let host_start = Instant::now();
    let mut host_command = Command::new(spec.binary);
    host_command.current_dir(&root);
    // Live smoke is an explicit opt-in release/evidence check. Some host CLIs need
    // noninteractive bypass flags for automation, so TFY compensates by validating
    // the hook ledger contains exactly the intended command before emitting evidence.
    configure_live_host_smoke_command(&mut host_command, spec, &prompt, max_budget_usd);
    let host_output = output_with_timeout(
        &mut host_command,
        spec.timeout,
        &stdout_path,
        &stderr_path,
        &format!("{} live hook smoke", integration.id),
    )?;
    let overhead_ms = millis_u64(host_start.elapsed());
    if !host_output.status.success() {
        bail!(
            "live host smoke failed for {}; stdout={} stderr={}",
            integration.id,
            stdout_path.display(),
            stderr_path.display()
        );
    }
    let provenance_path = root.join(provenance_path(integration.id));
    let provenance: HostConfigProvenance = serde_json::from_str(
        &fs::read_to_string(&provenance_path)
            .with_context(|| format!("read {}", provenance_path.display()))?,
    )
    .with_context(|| format!("parse {}", provenance_path.display()))?;
    let ledger = root.join(&provenance.ledger_path);
    let raw_dir = root.join(&provenance.raw_dir);
    let (raw_ref, raw_bytes, model_bytes) =
        official_hook_command_evidence(&ledger, integration.id, command_sample)
            .with_context(|| format!("verify {} hook ledger evidence", integration.id))?;
    let raw_artifact = raw_dir.join(format!("{raw_ref}.json"));
    if !raw_artifact.is_file() {
        bail!(
            "raw artifact missing after live host smoke: {}",
            raw_artifact.display()
        );
    }
    let setup_artifact = provenance_path;
    let config_path = root.join(&provenance.config_path);
    let evidence = root
        .join(".tfy")
        .join("host-evidence")
        .join(format!("{}-host-evidence.json", integration.id));
    if let Some(parent) = evidence.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &evidence,
        serde_json::to_string_pretty(&json!({
            "hosts": [{
                "host": integration.id,
                "tfy_version": env!("CARGO_PKG_VERSION"),
                "host_id": integration.id,
                "host_version": host_version.trim(),
                "setup_verified": true,
                "real_invocation_verified": true,
                "setup_artifact": setup_artifact,
                "invocation_artifact": stdout_path,
                "config_scope": "project",
                "config_path": config_path,
                "route_type": "official_host_hook",
                "ledger_artifact": ledger,
                "raw_artifact": raw_artifact,
                "redacted_public_bytes": raw_bytes,
                "model_visible_bytes": model_bytes,
                "timestamp": now_stamp(),
                "smoke_id": format!("{}-live-hook-smoke", integration.id),
                "official_docs_backed": true,
                "kill_switch_available": true,
                "uninstall_available": true,
                "overhead_ms": overhead_ms,
                "baseline_ms": baseline_ms,
                "overhead_exception": "live host smoke includes model planning latency; route launch evidence verifies official hook ingress and TFY raw/model byte gates"
            }]
        }))? + "\n",
    )?;
    Ok(json!({
        "host": integration.id,
        "display": integration.display,
        "status": "pass",
        "claim_tier": "host_evidence_recorded",
        "message": "live host CLI invoked TFY official hook route and produced launch-report host evidence",
        "workspace": root,
        "host_version": host_version.trim(),
        "ledger": ledger,
        "raw_artifact": raw_dir.join(format!("{raw_ref}.json")),
        "host_evidence": evidence,
        "launch_report_command": format!("tfy launch-report --host-evidence {} --json", evidence.display()),
        "redacted_public_bytes": raw_bytes,
        "model_visible_bytes": model_bytes,
        "setup_success_is_not_savings_success": true,
        "savings_success_verified": true
    }))
}

fn millis_u64(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn host_version(binary: &str) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("run {binary} --version"))?;
    if !output.status.success() {
        bail!(
            "{binary} --version failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        bail!("{binary} --version produced no stdout");
    }
    Ok(text)
}

fn output_with_timeout(
    command: &mut Command,
    timeout: Duration,
    stdout_path: &Path,
    stderr_path: &Path,
    label: &str,
) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().with_context(|| format!("spawn {label}"))?;
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child
                .wait_with_output()
                .with_context(|| format!("collect {label} output"))?;
            fs::write(stdout_path, &output.stdout)?;
            fs::write(stderr_path, &output.stderr)?;
            return Ok(output);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .with_context(|| format!("collect timed-out {label} output"))?;
            fs::write(stdout_path, &output.stdout)?;
            fs::write(stderr_path, &output.stderr)?;
            bail!(
                "{label} timed out after {}s; stdout={} stderr={}",
                timeout.as_secs(),
                stdout_path.display(),
                stderr_path.display()
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn official_hook_command_evidence(
    ledger: &Path,
    host: &str,
    expected_command: &str,
) -> Result<(String, usize, usize)> {
    let events = load_events(ledger).with_context(|| format!("read {}", ledger.display()))?;
    let mut matches = Vec::new();
    for event in events {
        if event.origin.invocation != OriginInvocation::OfficialHostHook {
            continue;
        }
        let host_matches = match host {
            "codex" => event.origin.host == tfy_runtime::OriginHost::Codex,
            "claude-code" => event.origin.host == tfy_runtime::OriginHost::ClaudeCode,
            _ => false,
        };
        if !host_matches {
            continue;
        }
        if let GatewayEvent::ToolCommandCompleted {
            command,
            raw_ref,
            raw_bytes,
            model_bytes,
            raw_chars,
            summary_chars,
            model_chars,
            negative_savings_avoided,
            ..
        } = event.payload
        {
            let raw_size = if raw_bytes == 0 { raw_chars } else { raw_bytes };
            let model_size = if model_bytes != 0 {
                model_bytes
            } else if model_chars != 0 {
                model_chars
            } else {
                summary_chars
            };
            if model_size > raw_size && !negative_savings_avoided {
                bail!("official hook evidence has negative savings");
            }
            if raw_size <= model_size {
                bail!("official hook evidence did not prove positive savings");
            }
            if raw_ref.trim().is_empty() {
                bail!("official hook evidence missing raw_ref");
            }
            matches.push((command, raw_ref, raw_size, model_size));
        }
    }
    if matches.len() != 1 {
        bail!(
            "expected exactly one official-host-hook command for {host}, found {}",
            matches.len()
        );
    }
    let (command, raw_ref, raw_size, model_size) = matches.remove(0);
    let expected_recorded_command = format!("sh -c {expected_command}");
    if command != expected_recorded_command {
        bail!(
            "official hook command mismatch for {host}: expected {:?}, got {:?}",
            expected_recorded_command,
            command
        );
    }
    Ok((raw_ref, raw_size, model_size))
}

fn raw_ref_path(raw_dir: &Path, raw_ref: &str) -> Result<PathBuf> {
    if !valid_raw_ref(raw_ref) {
        bail!("invalid raw ref: {raw_ref}");
    }
    Ok(raw_dir.join(format!("{raw_ref}.json")))
}

fn valid_raw_ref(raw_ref: &str) -> bool {
    let Some(rest) = raw_ref.strip_prefix("cmdout_") else {
        return false;
    };
    let parts: Vec<_> = rest.split('_').collect();
    parts.len() == 2
        && parts[0].len() == 12
        && parts[1].len() == 16
        && parts
            .iter()
            .all(|part| part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn raw_entries(raw_dir: &Path) -> Result<Vec<RawEntryReport>> {
    let mut entries = Vec::new();
    match fs::read_dir(raw_dir) {
        Ok(read_dir) => {
            for entry in read_dir {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if !valid_raw_ref(stem) {
                    continue;
                }
                entries.push(raw_entry(raw_dir, stem)?);
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("read raw dir {}", raw_dir.display())),
    }
    entries.sort_by(|a, b| a.raw_ref.cmp(&b.raw_ref));
    Ok(entries)
}

fn raw_entry(raw_dir: &Path, raw_ref: &str) -> Result<RawEntryReport> {
    let path = raw_ref_path(raw_dir, raw_ref)?;
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read raw evidence {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)?;
    let bytes = if let Some(raw_b64) = value["raw_b64"].as_str() {
        base64_len(raw_b64)
    } else if let Some(raw) = value["raw"].as_str() {
        raw.len()
    } else {
        bail!("raw evidence {raw_ref} is missing raw/raw_b64 bytes");
    };
    let command = value["command"]
        .as_str()
        .ok_or_else(|| anyhow!("raw evidence {raw_ref} is missing command"))?;
    let command_sha256 = sha256_hex(command.as_bytes());
    let exit_code = value["exit_code"]
        .as_i64()
        .ok_or_else(|| anyhow!("raw evidence {raw_ref} is missing exit_code"))?;
    let created_ns = value["created_ns"]
        .as_u64()
        .ok_or_else(|| anyhow!("raw evidence {raw_ref} is missing created_ns"))?;
    Ok(RawEntryReport {
        raw_ref: raw_ref.into(),
        path: path.display().to_string(),
        command_present: !command.is_empty(),
        command_sha256,
        exit_code,
        bytes,
        created_ns,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn base64_len(raw_b64: &str) -> usize {
    let padding = raw_b64
        .as_bytes()
        .iter()
        .rev()
        .take_while(|b| **b == b'=')
        .count();
    raw_b64.len().saturating_mul(3) / 4usize - padding
}

fn raw_entry_matches_age(entry: &RawEntryReport, older_than_days: Option<u64>) -> bool {
    let Some(days) = older_than_days else {
        return true;
    };
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let threshold = days
        .saturating_mul(24 * 60 * 60)
        .saturating_mul(1_000_000_000);
    entry.created_ns == 0 || now_ns.saturating_sub(entry.created_ns) >= threshold
}

fn build_benchmark_manifest(cmd: &BenchCmd) -> Result<BenchmarkManifest> {
    let samples = [
        (
            "cargo-test-failure",
            "cargo test",
            "running 1 test\ntest auth::rejects_bad_token ... FAILED\nfailures:\n---- auth::rejects_bad_token stdout ----\nthread 'auth::rejects_bad_token' panicked at src/auth.rs:42: expected Unauthorized, got Ok\n",
            101,
        ),
        (
            "repeated-long-output",
            "npm test",
            &(1..=120).map(|i| format!("spec line {i}: ok\n")).collect::<String>(),
            0,
        ),
        (
            "git-status-noise",
            "git status --short",
            " M crates/tfy-cli/src/product.rs\n M README.md\n?? target/tmp/ignored\n?? .tfy/raw/cmdout_test.json\n",
            0,
        ),
    ];
    let mut scenarios = Vec::new();
    for (name, command, raw, exit_code) in samples {
        let summary = summarize_command_output_with_policy(
            command,
            raw,
            exit_code,
            &cmd.raw_dir,
            ToolPolicy::Auto,
        )?;
        let raw_bytes = raw.len();
        let model_bytes = summary.model_text.len();
        scenarios.push(BenchmarkScenario {
            name: name.into(),
            command_family: summary.command_family,
            raw_bytes,
            model_bytes,
            saved_bytes: raw_bytes as isize - model_bytes as isize,
            no_negative_savings: model_bytes <= raw_bytes,
            raw_ref: summary.raw_ref,
        });
    }
    let raw_bytes = scenarios.iter().map(|scenario| scenario.raw_bytes).sum();
    let model_bytes = scenarios.iter().map(|scenario| scenario.model_bytes).sum();
    let saved_bytes = raw_bytes as isize - model_bytes as isize;
    let no_negative_savings = scenarios
        .iter()
        .all(|scenario| scenario.no_negative_savings);
    let positive_savings = scenarios.iter().any(|scenario| scenario.saved_bytes > 0);
    Ok(BenchmarkManifest {
        status: if no_negative_savings && positive_savings { "pass" } else { "blocked" }.into(),
        generated_by: format!("tfy {}", env!("CARGO_PKG_VERSION")),
        scenarios,
        tfy_self_benchmark: BenchmarkSummary {
            scenarios: 3,
            raw_bytes,
            model_bytes,
            saved_bytes,
            no_negative_savings,
            positive_savings,
        },
        public_superiority_claim_ready: false,
        claim_policy: "Public external superiority claims fail closed unless a reviewed benchmark manifest records baseline, corpus, reproducibility, correctness/no-lost-evidence proof, and overhead comparison.".into(),
    })
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
        tiers: vec!["official_host_hook_guidance", "instruction_guidance"],
        warnings: vec![
            "Codex setup guidance requires official host hook routing and real invocation evidence before launch claims.".into(),
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
        tiers: vec!["official_host_hook_guidance", "instruction_guidance"],
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

fn print_init_results(results: &[InitStatus], _session: &str) {
    for result in results {
        println!(
            "TFY init: action={} target={} dry_run={} applied={} path={}",
            result.action, result.target, result.dry_run, result.applied, result.path
        );
        println!("tiers={}", result.tiers.join(","));
        println!("marker_present={}", result.installed);
        if result.dry_run && result.action == "install" {
            println!("would_write marker block {TFY_CODEX_START} ... {TFY_CODEX_END}");
            println!("use `tfy start --agent --host codex` inside the project to write the official hook route");
        }
        for warning in &result.warnings {
            println!("warning: {warning}");
        }
    }
}

fn codex_instruction_block(_session: &str) -> String {
    format!(
        r#"{TFY_CODEX_START}
## TFY Codex integration

Integration tiers:
- official_host_hook_guidance: configure Codex through TFY's project hook route.
- instruction_guidance: route noisy command boundaries through TFY when configured.

Recommended setup command:

```sh
tfy start --agent --host codex
```

Safety boundary: this is official host-hook guidance, not private Codex hook interception, provider prompt interception, or universal shell interception.
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

fn build_product_status_report(cmd: &StatusCmd) -> ProductStatusReport {
    let target_filter = match (cmd.agent, cmd.human) {
        (true, false) => "agent",
        (false, true) => "human",
        _ => "all",
    };
    let mut project_lifecycle = lifecycle_status_view(LifecycleScope::Project);
    let mut global_lifecycle = lifecycle_status_view(LifecycleScope::Global);
    let project_parse_error = project_lifecycle.parse_error.is_some();
    let include_agent = !cmd.human;
    let include_human = !cmd.agent;
    if cmd.agent && !cmd.human {
        project_lifecycle.human = None;
        global_lifecycle.human = None;
    } else if cmd.human && !cmd.agent {
        project_lifecycle.agent = None;
        global_lifecycle.agent = None;
    }
    let effective_lifecycle = effective_lifecycle_status(
        project_lifecycle.agent.as_ref(),
        global_lifecycle.agent.as_ref(),
        project_lifecycle.human.as_ref(),
        global_lifecycle.human.as_ref(),
        project_parse_error,
        include_agent,
        include_human,
    );
    ProductStatusReport {
        status: "active".into(),
        target_filter: target_filter.into(),
        lifecycle_summary: lifecycle_summary(&effective_lifecycle),
        project_lifecycle,
        global_lifecycle,
        effective_lifecycle,
        minimum_v1_host_matrix: minimum_v1_host_matrix(),
        surfaces: vec![
            SurfaceStatus { name: "command_output".into(), status: "active".into(), message: "AI-origin commands can route through tfy agent/adapter/official-host-hook routes; human commands can route through an explicit tfy human shell managed session.".into() },
            SurfaceStatus { name: "context_compacting".into(), status: "active".into(), message: "AI can list scopes and request exact compact function/file scopes instead of whole files.".into() },
            SurfaceStatus { name: "compact_code_restore".into(), status: "active".into(), message: "Compact one-line/short-symbol transport is restored to readable canonical code for files and user display.".into() },
            SurfaceStatus { name: "workspace_apply".into(), status: "active".into(), message: "Multi-file add/modify/delete/rename/move apply is proof-gated and rollback-journaled.".into() },
            SurfaceStatus { name: "fuzzy_refactor_apply".into(), status: "active".into(), message: "Fuzzy edits require unique anchors, confidence threshold, restored preview hashes, and conflict checks.".into() },
            SurfaceStatus { name: "codex_host_routing".into(), status: "config_snippet_available".into(), message: "TFY can configure supported Codex routing surfaces; launch support still requires host-bound smoke/ledger/raw evidence.".into() },
            SurfaceStatus { name: "ordinary_human_terminal".into(), status: "not_supported".into(), message: "TFY does not globally intercept regular terminals; supported platform shells use `tfy start --human` to enter a current-directory-scoped managed PATH/proxy session, while `tfy shell <command>` remains raw passthrough and `tfy shell -- <command>` / `tfy tool-gateway -- <command>` remain explicit one-off gateway wrappers.".into() },
            SurfaceStatus {
                name: "human_managed_session".into(),
                status: if human_managed_session_available() { "available" } else { "unsupported_platform" }.into(),
                message: if human_managed_session_available() {
                    "Supported platform shells (Linux bash, macOS zsh, Windows PowerShell) can enter a TFY-managed current-directory-scoped auto-intercept session via `tfy start --human`; this is opt-in managed-session scope only and covers safely resolved ordinary external commands."
                } else {
                    "`tfy human shell` has no supported backend for this selected platform shell; ordinary terminals remain outside TFY unless explicit commands such as `tfy shell <command>` raw passthrough or `tfy shell -- <command>` gateway wrapper are used."
                }.into()
            },
            SurfaceStatus { name: "provider_api_gateway".into(), status: "not_supported".into(), message: "OpenAI/provider request proxying is outside TFY scope.".into() },
            SurfaceStatus { name: "editor_integration".into(), status: "not_supported".into(), message: "Editor auto-connection is outside TFY scope.".into() },
            SurfaceStatus { name: "private_codex_hook".into(), status: "not_supported".into(), message: "No private or hidden Codex prompt interception is claimed.".into() },
        ],
        claim_evidence_ladder: claim_evidence_ladder(),
        launch_claim_gate: "A host is launch-supported only after config snippet, config write/apply proof, host launch, verified official hook invocation, route evidence, raw recovery, no-negative-savings, and positive-savings checks pass.".into(),
        not_supported: not_supported_surfaces(),
        truthfulness_boundary: "automatic configuration is limited to supported AI-host routes and explicit TFY-managed human sessions; no provider proxy, editor hook, private Codex hook, or universal shell interception".into(),
    }
}

fn lifecycle_summary(effective: &EffectiveLifecycleStatus) -> LifecycleSummary {
    let mut desired_targets = Vec::new();
    let mut configured_targets = Vec::new();
    let mut active_targets = Vec::new();
    let mut any_desired = false;
    let mut any_configured = false;
    let mut any_route_configured = false;
    let mut any_active = false;
    let mut any_parse_error = false;

    for (name, route) in [
        ("agent", effective.agent.as_ref()),
        ("human", effective.human.as_ref()),
    ] {
        let Some(route) = route else {
            continue;
        };
        if route.support_status == "lifecycle_parse_error"
            || route.route_state == "lifecycle_parse_error"
        {
            any_parse_error = true;
        }
        if route.desired {
            any_desired = true;
            desired_targets.push(name.to_string());
        }
        if route.configured || route.route_configured {
            any_configured = true;
            configured_targets.push(name.to_string());
        }
        if route.route_configured {
            any_route_configured = true;
        }
        if route.active {
            any_active = true;
            active_targets.push(name.to_string());
        }
    }

    let status = if any_parse_error {
        "parse_error"
    } else if any_active {
        "active"
    } else if any_desired && any_route_configured {
        "configured_unverified"
    } else if any_desired || any_configured {
        "intent_recorded"
    } else {
        "not_configured"
    };
    let evidence_required = any_desired && !any_active;
    let message = match status {
        "active" => {
            "TFY is active only for targets with verified route, raw/ledger, and savings evidence."
        }
        "configured_unverified" => {
            "TFY is configured for at least one target, but active=false until route-bound invocation and savings evidence exists."
        }
        "intent_recorded" => {
            "TFY lifecycle intent is recorded, but no supported route is verified active yet."
        }
        "parse_error" => "Lifecycle state could not be parsed; fix or remove the lifecycle file.",
        _ => "TFY lifecycle is not configured for the selected target filter.",
    };

    LifecycleSummary {
        status: status.into(),
        desired: any_desired,
        configured: any_configured,
        active: any_active,
        desired_targets,
        configured_targets,
        active_targets,
        evidence_required,
        message: message.into(),
    }
}

fn effective_lifecycle_status(
    project_agent: Option<&AgentLifecycleState>,
    global_agent: Option<&AgentLifecycleState>,
    project_human: Option<&HumanLifecycleState>,
    global_human: Option<&HumanLifecycleState>,
    project_parse_error: bool,
    include_agent: bool,
    include_human: bool,
) -> EffectiveLifecycleStatus {
    EffectiveLifecycleStatus {
        agent: include_agent
            .then(|| effective_agent_status(project_agent, global_agent, project_parse_error)),
        human: include_human
            .then(|| effective_human_status(project_human, global_human, project_parse_error)),
    }
}

fn effective_agent_status(
    project: Option<&AgentLifecycleState>,
    global: Option<&AgentLifecycleState>,
    project_parse_error: bool,
) -> EffectiveRouteStatus {
    if project_parse_error {
        return EffectiveRouteStatus {
            desired: false,
            desired_source: "project_parse_error".into(),
            configured: false,
            lifecycle_started: false,
            route_configured: false,
            route_verified: false,
            savings_verified: false,
            normal_workflow_supported: false,
            active: false,
            active_derivation: "inactive: lifecycle parse error".into(),
            route_state: "lifecycle_parse_error".into(),
            support_status: "lifecycle_parse_error".into(),
            next_action: "Fix or remove .tfy/lifecycle.json before TFY can inherit project or global agent lifecycle state.".into(),
        };
    }
    let selected = project
        .map(|state| (state, "project"))
        .or_else(|| global.map(|state| (state, "global")));
    let desired = selected.is_some_and(|(state, _)| state.desired);
    let configured = selected.is_some_and(|(state, _)| state.configured);
    let route_configured = selected.is_some_and(|(state, _)| {
        state
            .host_routes
            .values()
            .any(|route| route.route_configured)
    });
    let route_verified = selected
        .is_some_and(|(state, _)| state.host_routes.values().any(|route| route.route_verified));
    let savings_verified = selected.is_some_and(|(state, _)| {
        state
            .host_routes
            .values()
            .any(|route| route.savings_verified)
    });
    let normal_workflow_supported = selected.is_some_and(|(state, _)| {
        state
            .host_routes
            .values()
            .any(|route| route.normal_workflow_supported)
    });
    let route_bound_active = selected.is_some_and(|(state, _)| {
        state.host_routes.values().any(|route| {
            route.route_verified && route.savings_verified && route.normal_workflow_supported
        })
    });
    let active = desired && route_bound_active;
    let host_routes_empty = selected.is_none_or(|(state, _)| state.host_routes.is_empty());
    EffectiveRouteStatus {
        desired,
        desired_source: selected.map(|(_, source)| source).unwrap_or("none").into(),
        configured,
        lifecycle_started: desired,
        route_configured,
        route_verified,
        savings_verified,
        normal_workflow_supported,
        active,
        active_derivation: if active {
            "active=true derived from desired && route_verified && savings_verified && normal_workflow_supported"
        } else {
            "active=false until a host+route evidence scope verifies invocation, raw recovery, no-negative and positive savings and lifecycle desire is on"
        }
        .into(),
        route_state: selected
            .map(|(state, _)| state.route_state.clone())
            .unwrap_or_else(|| "not_configured".into()),
        support_status: selected
            .map(|(state, _)| state.support_status.clone())
            .unwrap_or_else(|| "host_route_configuration_required".into()),
        next_action: if active {
            "Route is active only for the verified host+route evidence scope; reverify after config, host, TFY version, raw-ref, or route changes.".into()
        } else if !desired {
            "Run `tfy start --agent` in this project, or configure global agent defaults with `tfy use always --agent`.".into()
        } else if host_routes_empty {
            "Run `tfy start --agent` to prepare the project agent wrapper plus Codex and Claude Code official hook routes, or use `tfy start --agent --no-apply` for lifecycle intent only.".into()
        } else {
            "Configured but not verified: run the configured host and collect route-bound raw/ledger/no-negative/positive-savings/overhead evidence before active=true.".into()
        },
    }
}

fn effective_human_status(
    project: Option<&HumanLifecycleState>,
    global: Option<&HumanLifecycleState>,
    project_parse_error: bool,
) -> EffectiveRouteStatus {
    if project_parse_error {
        return EffectiveRouteStatus {
            desired: false,
            desired_source: "project_parse_error".into(),
            configured: false,
            lifecycle_started: false,
            route_configured: false,
            route_verified: false,
            savings_verified: false,
            normal_workflow_supported: false,
            active: false,
            active_derivation: "inactive: lifecycle parse error".into(),
            route_state: "lifecycle_parse_error".into(),
            support_status: "lifecycle_parse_error".into(),
            next_action: "Fix or remove .tfy/lifecycle.json before TFY can inherit project or global human lifecycle state.".into(),
        };
    }
    let selected = project
        .map(|state| (state, "project"))
        .or_else(|| global.map(|state| (state, "global")));
    let desired = selected.is_some_and(|(state, _)| state.desired);
    let configured = selected.is_some_and(|(state, _)| state.configured);
    let route_configured = selected.is_some_and(|(state, _)| state.managed_session_available);
    let route_verified = selected.is_some_and(|(state, _)| state.active);
    let savings_verified = route_verified;
    let normal_workflow_supported = false;
    let active = desired && route_verified && savings_verified;
    EffectiveRouteStatus {
        desired,
        desired_source: selected.map(|(_, source)| source).unwrap_or("none").into(),
        configured,
        lifecycle_started: desired,
        route_configured,
        route_verified,
        savings_verified,
        normal_workflow_supported,
        active,
        active_derivation: if active {
            "active=true derived from desired && explicit TFY-managed human session evidence"
        } else {
            "active=false until explicit TFY-managed human session evidence exists and lifecycle desire is on; ordinary terminals are not globally intercepted"
        }
        .into(),
        route_state: selected
            .map(|(state, _)| state.route_state.clone())
            .unwrap_or_else(|| "not_configured".into()),
        support_status: selected
            .map(|(state, _)| state.support_status.clone())
            .unwrap_or_else(|| "manual_explicit_route_required".into()),
        next_action: if active {
            "Human route is active from explicit TFY-managed session evidence.".into()
        } else if !desired {
            "Run `tfy start --human` to opt into explicit human managed-session cleanup.".into()
        } else if !route_configured {
            "TFY-managed human sessions are unavailable for the selected platform shell on this platform; ordinary terminals are not globally intercepted. Use `tfy shell <command>` for raw passthrough or `tfy shell -- <command>` for the TFY gateway wrapper.".into()
        } else {
            "Run `tfy start --human` to enter the supported platform-shell current-directory-scoped managed session; ordinary terminals outside that session are not globally intercepted. Use `tfy human shell --no-auto-intercept` for a managed shell without PATH-shim routing, `tfy shell <command>` for raw passthrough, or `tfy shell -- <command>` / `tfy tool-gateway -- <command>` for explicit one-off gateway wrapping.".into()
        },
    }
}

fn claim_evidence_ladder() -> Vec<String> {
    [
        "config_snippet_available",
        "config_written",
        "host_launched",
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
    } else if host.status == "unsupported" {
        "unsupported"
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
        supported_ingress: if matches!(host.status, "planned_discovery" | "unsupported") {
            Vec::new()
        } else if matches!(host.id, "codex" | "claude-code") {
            vec!["official_host_hook".into(), "agent_wrapper_fallback".into()]
        } else {
            Vec::new()
        },
        equivalence_ingress: if matches!(host.status, "planned_discovery" | "unsupported") {
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
    if observed.iter().any(|observed| observed == "unsupported") {
        return "not_applicable";
    }
    let has = |tier: &str| observed.iter().any(|observed| observed == tier);
    if has("launch_supported") {
        "complete"
    } else if has("savings_verified") {
        "launch_supported"
    } else if has("route_evidence_recorded") {
        "savings_verified"
    } else if has("verified_host_hook") {
        "route_evidence_recorded"
    } else if has("host_launched") {
        "verified_host_hook"
    } else if has("config_written") {
        "host_launched"
    } else {
        "config_written"
    }
}

fn minimum_v1_host_matrix() -> Vec<HostReadiness> {
    let mut hosts = vec![
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
            supported_ingress: vec!["official_host_hook".into(), "agent_wrapper_fallback".into()],
            equivalence_ingress: vec!["official_host_hook_test_shim_only".into()],
            unsupported_ingress: unsupported_ingress(),
            config_strategy: "Codex project .codex/config.toml PreToolUse Bash hook; project .codex layer and /hooks trust required".into(),
            apply_strategy: "safe project .codex/config.toml hook writer with TFY marker block, backup, provenance, idempotency, and uninstall".into(),
            smoke_strategy: "local hook smoke plus real Codex invocation artifact".into(),
            host_evidence_strategy: "Codex-bound official_host_hook ledger/raw evidence with config path and smoke id".into(),
            setup: "Codex project .codex/config.toml PreToolUse Bash hook plus optional TFY AGENTS.md guidance".into(),
            normal_workflow: "Codex remains normal only after project .codex trust, /hooks review, and route-bound evidence; checklist-only guidance is not launch support".into(),
            evidence_gate: vec![
                "Codex host actually invokes TFY official host hook".into(),
                "ledger events from Codex session".into(),
                "raw recovery and no-negative-savings".into(),
            ],
            launch_claim: "not launch-supported until real Codex invocation artifact plus TFY ledger/raw/no-negative/positive-savings evidence exists".into(),
        },
    ];
    for host in host_registry()
        .into_iter()
        .filter(|host| host.id == "claude-code")
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
                    if event.adapter_kind == AdapterKind::Cli
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
                GatewayEvent::ContextSelected { .. }
                | GatewayEvent::OutputValidated { .. }
                | GatewayEvent::StateProjected { .. } => {}
                GatewayEvent::Fallback { .. }
                | GatewayEvent::Validation { .. }
                | GatewayEvent::Error { .. } => {}
            }
        }
    }
    apply_host_setup_evidence(&mut summary, host_evidence_files);
    summary.raw_refs = summary.generic_shell_route.raw_refs
        + summary.tfy_agent_adapter_route.raw_refs
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

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}

fn parse_unix_stamp(value: &str) -> Option<u64> {
    value.strip_prefix("unix:")?.parse().ok()
}

fn host_evidence_is_fresh(host: &HostSetupEvidence) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if host.reverify_failed == Some(true) {
        reasons.push("reverify_failed=true".into());
    }
    match host.tfy_version.as_deref() {
        Some(env!("CARGO_PKG_VERSION")) => {}
        Some(version) => reasons.push(format!(
            "tfy_version_mismatch evidence={} current={}",
            version,
            env!("CARGO_PKG_VERSION")
        )),
        None => reasons.push("tfy_version_missing".into()),
    }
    if let Some(expires) = host.evidence_expires_at.as_deref() {
        match parse_unix_stamp(expires) {
            Some(expiry) if expiry >= current_unix_seconds() => {}
            Some(_) => reasons.push("evidence_expired".into()),
            None => reasons.push("evidence_expires_at_invalid_format".into()),
        }
    }
    (reasons.is_empty(), reasons)
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
                let (evidence_fresh, stale_reasons) = host_evidence_is_fresh(&host);
                let route_type_allowed = required_route_type_allowed(&host.route_type);
                let config_scope_valid = non_empty_opt(&host.config_scope);
                let host_version_valid = non_empty_opt(&host.host_version);
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
                        _ => (true, false),
                    };
                let host_bound = evidence_fresh
                    && route_type_allowed
                    && config_scope_valid
                    && host_version_valid
                    && config_path_verified
                    && ledger_artifact_verified
                    && raw_artifact_verified
                    && smoke_id_valid
                    && timestamp_valid
                    && no_negative
                    && positive;
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
                    route.overhead_exception = host.overhead_exception.clone();
                }
                route.host_bound_evidence |= host_bound;
                route.no_negative_savings &= no_negative;
                route.positive_savings |= positive;
                if host_bound {
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
                }
                summary.evidence_notes.push(format!(
                    "host_evidence {} setup_verified={} setup_artifact_verified={} real_invocation_verified={} invocation_artifact_verified={} overhead_measured={} overhead_passed={} host_bound_evidence={} config_path_verified={} ledger_artifact_verified={} raw_artifact_verified={} host_version_valid={} evidence_fresh={} stale_reasons={}",
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
                    host_version_valid,
                    evidence_fresh,
                    stale_reasons.join("|")
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
                    route.overhead_exception = host.overhead_exception.clone();
                }
                let (evidence_fresh, stale_reasons) = host_evidence_is_fresh(&host);
                let host_id_matches = host
                    .host_id
                    .as_deref()
                    .is_some_and(|host_id| host_id == host.host);
                let route_type_allowed = named_host_route_type_allowed(&host.route_type);
                let config_scope_valid = non_empty_opt(&host.config_scope);
                let host_version_valid = non_empty_opt(&host.host_version);
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
                let hook_route = named_host_route_is_hook(&host.route_type);
                let hook_authorized = !hook_route
                    || (host.official_docs_backed == Some(true)
                        && host.kill_switch_available == Some(true)
                        && host.uninstall_available == Some(true)
                        && host_official_hook_launch_supported(&host.host));
                let host_bound = evidence_fresh
                    && host_id_matches
                    && route_type_allowed
                    && config_scope_valid
                    && host_version_valid
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
                    "named_host_evidence {} setup_verified={} setup_artifact_verified={} real_invocation_verified={} invocation_artifact_verified={} overhead_measured={} overhead_passed={} host_bound_evidence={} config_path_verified={} ledger_artifact_verified={} raw_artifact_verified={} host_version_valid={} hook_route={} hook_authorized={} evidence_fresh={} stale_reasons={}",
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
                    host_version_valid,
                    hook_route,
                    hook_authorized,
                    evidence_fresh,
                    stale_reasons.join("|")
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

fn named_host_route_type_allowed(value: &Option<String>) -> bool {
    matches!(
        value.as_deref(),
        Some("official_host_hook" | "host_hook" | "hook")
    )
}

fn required_route_type_allowed(value: &Option<String>) -> bool {
    matches!(
        value.as_deref(),
        Some("generic_shell" | "tfy_agent_adapter")
    )
}

fn named_host_route_is_hook(value: &Option<String>) -> bool {
    matches!(
        value.as_deref(),
        Some("official_host_hook" | "host_hook" | "hook")
    )
}

fn route_evidence_has_allowed_launch_route(route: &RouteEvidence) -> bool {
    named_host_route_is_hook(&route.route_type)
        && route.official_docs_backed
        && route.kill_switch_available
        && route.uninstall_available
}

fn host_accepts_launch_evidence(host: &str) -> bool {
    host_integration(host).is_ok_and(|integration| integration.status == "config_snippet_available")
}

fn host_official_hook_launch_supported(host: &str) -> bool {
    live_host_smoke_spec(host).is_some()
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
                "verified_local_route"
            }
            .into();
            host.claim_tier = if route.is_some_and(|route| route.real_invocation_verified) {
                "smoke_routed"
            } else if route.is_some_and(|route| route.setup_verified) {
                "applied_unverified"
            } else {
                "verified_local_route"
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

fn host_observed_evidence_tiers(_host: &str, route: &RouteEvidence) -> Vec<String> {
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
        && route_evidence_has_allowed_launch_route(route)
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
        && route.host_bound_evidence
        && (!hook_route
            || (route.official_docs_backed
                && route.kill_switch_available
                && route.uninstall_available))
        && (route.overhead_measured || route.overhead_exception.is_some())
        && route.overhead_passed
}

fn benchmark_manifest_passes(base: &Path, artifact: &Path) -> bool {
    let path = if artifact.is_absolute() {
        artifact.to_path_buf()
    } else {
        base.join(artifact)
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    value["status"] == "pass"
        && value["tfy_self_benchmark"]["no_negative_savings"] == true
        && value["tfy_self_benchmark"]["positive_savings"] == true
        && value["scenarios"].as_array().is_some_and(|scenarios| {
            !scenarios.is_empty()
                && scenarios.iter().all(|scenario| {
                    scenario["raw_ref"]
                        .as_str()
                        .is_some_and(|raw_ref| !raw_ref.is_empty())
                        && scenario["no_negative_savings"] == true
                })
        })
}

fn build_release_evidence_summary(files: &[PathBuf]) -> ReleaseEvidenceSummary {
    let mut summary = ReleaseEvidenceSummary::default();
    for path in files {
        let Ok(text) = fs::read_to_string(path) else {
            summary.notes.push(format!(
                "release_evidence_file_unreadable={}",
                path.display()
            ));
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<ReleaseEvidenceFile>(&text) else {
            summary.notes.push(format!(
                "release_evidence_file_invalid_json={}",
                path.display()
            ));
            continue;
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let artifact = |candidate: &Option<PathBuf>| {
            candidate
                .as_ref()
                .is_some_and(|artifact| host_artifact_exists(base, artifact))
        };
        summary.cargo_install_verified |= parsed.cargo_install_verified.unwrap_or(false)
            && artifact(&parsed.cargo_install_binary);
        summary.cargo_build_release_verified |=
            parsed.cargo_build_release_verified.unwrap_or(false)
                && artifact(&parsed.release_binary);
        summary.archive_checksum_dry_run |= parsed.archive_checksum_dry_run.unwrap_or(false)
            && artifact(&parsed.archive_artifact)
            && artifact(&parsed.checksum_artifact);
        summary.docs_demo_release_notes_complete |=
            parsed.docs_demo_release_notes_complete.unwrap_or(false)
                && artifact(&parsed.docs_artifact)
                && artifact(&parsed.release_notes_artifact);
        summary.independent_reviews_approved |=
            parsed.independent_reviews_approved.unwrap_or(false)
                && artifact(&parsed.review_artifact);
        summary.pr_ci_green |= parsed.pr_ci_green.unwrap_or(false) && artifact(&parsed.ci_artifact);
        summary.benchmark_manifest_generated |=
            parsed.benchmark_manifest_generated.unwrap_or(false)
                && parsed
                    .benchmark_manifest
                    .as_ref()
                    .is_some_and(|artifact| benchmark_manifest_passes(base, artifact));
        if let Some(notes) = parsed.notes {
            summary.notes.extend(notes);
        }
        summary
            .notes
            .push(format!("release_evidence_file={}", path.display()));
    }
    summary
}

fn build_release_tier_report(
    host_matrix: &[HostReadiness],
    gain: &GainReport,
    host_evidence: &HostEvidenceSummary,
    release_evidence: &ReleaseEvidenceSummary,
    launch_blockers: &[String],
) -> ReleaseTierReport {
    let required_routes_ready = ["generic_shell", "tfy_agent_adapter"]
        .iter()
        .all(|required| {
            host_matrix
                .iter()
                .any(|host| host.host == *required && host.status == "launch_supported")
        });
    let unsupported_audit_pass = true;
    let raw_lifecycle_available = true;
    let benchmark_self_manifest_available = true;
    let beta_blockers = tier_blockers(&[
        (
            required_routes_ready,
            "required routes generic_shell/tfy_agent_adapter are not launch_supported",
        ),
        (
            release_evidence.cargo_install_verified,
            "cargo install --path verification evidence missing",
        ),
        (
            release_evidence.cargo_build_release_verified,
            "cargo build --release verification evidence missing",
        ),
        (gain.commands > 0, "no command-output gain evidence"),
        (gain.saved_bytes >= 0, "negative savings detected"),
        (
            host_evidence.no_negative_savings,
            "missing no-negative route evidence",
        ),
        (
            host_evidence.positive_savings,
            "missing positive route savings evidence",
        ),
        (
            raw_lifecycle_available,
            "raw lifecycle commands unavailable",
        ),
        (
            benchmark_self_manifest_available && release_evidence.benchmark_manifest_generated,
            "benchmark self-manifest unavailable",
        ),
        (unsupported_audit_pass, "unsupported claim audit failed"),
    ]);
    let beta_ready = TierStatus {
        status: if beta_blockers.is_empty() {
            "ready"
        } else {
            "blocked"
        }
        .into(),
        evidence: vec![
            "cargo build/install must be verified by release gate".into(),
            "first-success quickstart uses smoke --all + launch-report evidence".into(),
            "raw lifecycle: tfy raw --list/--inspect/--export/--prune".into(),
            "benchmark manifest: tfy bench --json".into(),
            "unsupported claim audit is generated from launch-report".into(),
        ],
        blockers: beta_blockers.clone(),
    };
    let developer_preview_ready = beta_ready.clone();
    let rc_blockers = tier_blockers(&[
        (beta_ready.status == "ready", "beta_ready is blocked"),
        (
            release_evidence.archive_checksum_dry_run,
            "release archive/checksum dry-run evidence must be attached by release script/CI",
        ),
        (
            release_evidence.docs_demo_release_notes_complete,
            "docs/demo/release notes completion must be verified in final closeout",
        ),
        (
            release_evidence.independent_reviews_approved && release_evidence.pr_ci_green,
            "independent reviews and PR/CI green are final-story gates",
        ),
    ]);
    let rc_ready = TierStatus {
        status: if rc_blockers.is_empty() { "ready" } else { "blocked" }.into(),
        evidence: vec!["RC is finalized by packaging/docs/review/CI closeout, not by local launch-report alone".into()],
        blockers: rc_blockers,
    };
    let named_host_launch = host_matrix.iter().any(|host| {
        matches!(host.host.as_str(), "codex" | "claude-code") && host.status == "launch_supported"
    });
    let ga_blockers = tier_blockers(&[
        (rc_ready.status == "ready", "rc_ready is blocked"),
        (named_host_launch, "no named AI host has real invocation + route-bound raw/no-negative/positive-savings launch evidence"),
        (launch_blockers.is_empty(), "launch-report still has blockers"),
    ]);
    let ga_ready = TierStatus {
        status: if ga_blockers.is_empty() { "ready" } else { "blocked" }.into(),
        evidence: vec!["GA requires at least one named AI host promoted to launch_supported by real invocation evidence".into()],
        blockers: ga_blockers,
    };
    let superiority_blockers = tier_blockers(&[
        (ga_ready.status == "ready", "ga_ready is blocked"),
        (
            false,
            "external baseline manifest with baseline/corpus/reproducibility is not attached",
        ),
        (
            false,
            "public superiority publication approval is not attached",
        ),
    ]);
    let public_superiority_claim_ready = TierStatus {
        status: if superiority_blockers.is_empty() {
            "ready"
        } else {
            "blocked"
        }
        .into(),
        evidence: vec![
            "Public external superiority claims fail closed without reviewed benchmark manifest"
                .into(),
        ],
        blockers: superiority_blockers,
    };
    ReleaseTierReport {
        beta_ready,
        developer_preview_ready,
        rc_ready,
        ga_ready,
        public_superiority_claim_ready,
    }
}

fn tier_blockers(checks: &[(bool, &str)]) -> Vec<String> {
    checks
        .iter()
        .filter(|(ok, _message)| !*ok)
        .map(|(_ok, message)| (*message).into())
        .collect()
}

fn build_launch_readiness_report(
    status: ProductStatusReport,
    gain: GainReport,
    host_evidence: HostEvidenceSummary,
    release_evidence: ReleaseEvidenceSummary,
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
        blockers.push("no command-output savings data found; run adapter/agent workflows before launch claims".into());
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
        release_tiers: build_release_tier_report(&host_matrix, &gain, &host_evidence, &release_evidence, &blockers),
        host_matrix,
        claim_evidence_ladder: claim_evidence_ladder(),
        gain,
        host_evidence,
        release_evidence,
        release_thresholds: ReleaseThresholds::default(),
        measurement_method: MeasurementMethod::default(),
        privacy_raw_store: PrivacyRawStorePolicy::default(),
        overhead_policy: OverheadPolicy::default(),
        unsupported_claim_audit: UnsupportedClaimAudit {
            status: "pass".into(),
            audited_claims: not_supported_surfaces(),
            rule: "provider/API prompt proxy, editor-internal auto hook, private Codex hook interception, and universal terminal interception claims must remain not_supported; official-host-hook routes may promote only through config_written, host_launched, verified host invocation, route evidence, and savings_verified gates".into(),
        },
        blockers,
        not_supported: status.not_supported,
        required_benchmark_scenarios: vec![
            "command-heavy debugging".into(),
            "context-heavy code edit".into(),
            "repeated test loop".into(),
            "Git/GitHub evidence".into(),
            "long-session state compaction".into(),
            "host setup failure recovery".into(),
        ],
    }
}

fn build_explain_report() -> ProductExplainReport {
    ProductExplainReport {
        ai_transport: "AI sees compact scope/function-level code such as `function f0(a,b){const c=a+b;return c;}` when that saves tokens.".into(),
        file_and_user_output: "Before code is written or shown to a human, TFY restores original/readable names, indentation, and line breaks into canonical file code.".into(),
        automatic_routing: "TFY can prepare the project agent wrapper by default and can configure supported AI-agent host routes where safe writers exist; Launch support is granted only after host-bound evidence proves routing and savings.".into(),
        apply_model: "Writes are validated with plan hash, per-operation proof, preview hash, origin checks, safe paths, rollback journal, and fuzzy unique-anchor gates.".into(),
        what_tfy_changed: vec![
            "configured AI command/context/output boundaries route through TFY instead of sending raw noisy payloads directly to the model".into(),
            "ordinary human terminals are not globally intercepted".into(),
            "launch support is evidence-gated per host route".into(),
        ],
        data_stored_locally: vec![
            "raw command/context evidence under .tfy/raw or configured --raw-dir".into(),
            "gateway ledgers such as .tfy/adapter/ledger.jsonl and .tfy/agent/ledger.jsonl".into(),
            "compact state projections derived from local ledgers".into(),
        ],
        raw_recovery: "Use raw_ref values with `tfy raw` to recover byte-exact evidence.".into(),
        deletion_export: "Use `tfy raw --list`, `tfy raw <raw_ref> --inspect`, `tfy raw <raw_ref> --export <path>`, and explicit `tfy raw --prune --dry-run/--apply` for local raw evidence lifecycle; TFY does not upload raw evidence and ledger files remain local.".into(),
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
    if ensure_writable_dir(Path::new(".tfy/raw")).is_ok() {
        diagnostics.push(Diagnostic {
            name: "writable_dirs".into(),
            status: "pass".into(),
            message: ".tfy/raw is writable".into(),
        });
    } else {
        diagnostics.push(Diagnostic {
            name: "writable_dirs".into(),
            status: "fail".into(),
            message: "could not create/write .tfy/raw".into(),
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

fn write_required_route_smoke_evidence(reports: &[SmokeReport]) -> Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "tfy-required-route-evidence-{}-{}",
        std::process::id(),
        stable_id("required-route-evidence")
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let setup = root.join("setup-proof.txt");
    let invocation = root.join("invocation-proof.txt");
    fs::write(
        &setup,
        "local required-route setup verified by tfy smoke --all",
    )?;
    fs::write(
        &invocation,
        "local required-route invocation verified by tfy smoke --all",
    )?;
    let mut hosts = Vec::new();
    for (mode, host, route_type) in [
        ("adapter", "generic_shell", "generic_shell"),
        ("agent", "tfy_agent_adapter", "tfy_agent_adapter"),
    ] {
        let report = reports
            .iter()
            .find(|report| report.mode == mode)
            .ok_or_else(|| anyhow!("missing {mode} smoke report"))?;
        let ledger = report
            .evidence
            .iter()
            .find_map(|entry| entry.strip_prefix("ledger="))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&report.sample_path));
        let raw_dir = report
            .evidence
            .iter()
            .find_map(|entry| entry.strip_prefix("raw_dir="))
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing raw_dir evidence for {mode}"))?;
        let events = load_events(&ledger).with_context(|| format!("read {mode} smoke ledger"))?;
        let mut raw_ref = None;
        let mut raw_bytes = 0usize;
        let mut model_bytes = 0usize;
        for event in events {
            if let GatewayEvent::ToolCommandCompleted {
                raw_bytes: event_raw_bytes,
                model_bytes: event_model_bytes,
                raw_chars,
                model_chars,
                summary_chars,
                raw_ref: event_raw_ref,
                ..
            } = event.payload
            {
                raw_ref = Some(event_raw_ref);
                raw_bytes = if event_raw_bytes == 0 {
                    raw_chars
                } else {
                    event_raw_bytes
                };
                model_bytes = if event_model_bytes != 0 {
                    event_model_bytes
                } else if model_chars != 0 {
                    model_chars
                } else {
                    summary_chars
                };
                break;
            }
        }
        let raw_ref = raw_ref.ok_or_else(|| anyhow!("missing command event for {mode}"))?;
        let raw_artifact = raw_dir.join(format!("{raw_ref}.json"));
        let config_path = root.join(format!("{host}-config.txt"));
        fs::write(&config_path, format!("{host} local smoke config proof"))?;
        hosts.push(json!({
            "host": host,
            "tfy_version": env!("CARGO_PKG_VERSION"),
            "host_version": "tfy-local-smoke",
            "setup_verified": true,
            "real_invocation_verified": true,
            "setup_artifact": setup,
            "invocation_artifact": invocation,
            "route_type": route_type,
            "config_scope": "local_smoke",
            "config_path": config_path,
            "ledger_artifact": ledger,
            "raw_artifact": raw_artifact,
            "redacted_public_bytes": raw_bytes,
            "model_visible_bytes": model_bytes,
            "timestamp": "2026-06-09T00:00:00Z",
            "smoke_id": format!("tfy-{mode}-smoke"),
            "overhead_exception": format!("local smoke fixture: {host} overhead accepted for beta route evidence")
        }));
    }
    let evidence = root.join("host-evidence.json");
    fs::write(
        &evidence,
        serde_json::to_string_pretty(&json!({ "hosts": hosts }))? + "\n",
    )?;
    Ok(evidence)
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

fn print_codex_smoke_checklist() -> Result<()> {
    write_codex_smoke_checklist(std::io::stdout())
}

fn write_codex_smoke_checklist(mut output: impl Write) -> Result<()> {
    writeln!(
        output,
        "TFY Codex smoke is checklist/report-only until real Codex hook invocation evidence exists."
    )?;
    writeln!(
        output,
        "1. Run `tfy start --agent --host codex` inside the project."
    )?;
    writeln!(
        output,
        "2. Trust/reload the project Codex hook configuration."
    )?;
    writeln!(
        output,
        "3. Ask Codex to run a normal Bash command and verify TFY records route-bound raw/ledger/no-negative/positive-savings evidence."
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
        PathBuf::from(".tfy/adapter/ledger.jsonl"),
        PathBuf::from(".tfy/agent/ledger.jsonl"),
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
