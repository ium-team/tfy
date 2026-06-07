use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub const RUNTIME_PROTOCOL_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayKind {
    Tool,
    Context,
    Output,
    State,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Shell,
    Mcp,
    Codex,
    Editor,
    Provider,
    TestHarness,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityMode {
    ObserveOnly,
    Suggest,
    ExecuteWithRuntimeAuthority,
    RequiresUserConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Text,
    Json,
    Jsonl,
    McpResource,
    ProviderPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    UnsupportedProtocolVersion,
    UnsupportedGateway,
    UnsupportedOutputMode,
    AdapterDisabled,
    MissingRawStore,
    RedactionUnavailable,
    InsufficientAuthority,
    MissingProvenance,
    StaleProvenance,
    ValidationFailed,
    LowConfidence,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    NotValidated,
    Valid,
    Invalid,
    Stale,
    NonAuthoritative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProvenanceRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patch_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ledger_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_event_ids: Vec<String>,
    pub validation_status: Option<ValidationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePolicy {
    pub max_tokens: Option<usize>,
    pub max_output_bytes: usize,
    pub redact_public: bool,
    pub require_fallback_refs: bool,
    pub destructive_action_requires_confirmation: bool,
    pub adapter_enabled: bool,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            max_tokens: None,
            max_output_bytes: 1_000_000,
            redact_public: true,
            require_fallback_refs: true,
            destructive_action_requires_confirmation: true,
            adapter_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEnvelope<T> {
    pub protocol_version: String,
    pub adapter_kind: AdapterKind,
    pub adapter_version: String,
    pub supported_gateways: Vec<GatewayKind>,
    pub authority_mode: AuthorityMode,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub request_id: String,
    pub parent_event_id: Option<String>,
    pub trace_id: String,
    pub workspace_root: Option<String>,
    pub provenance: ProvenanceRefs,
    pub policy: RuntimePolicy,
    pub payload: T,
}

impl<T> RuntimeEnvelope<T> {
    pub fn new(
        adapter_kind: AdapterKind,
        supported_gateways: Vec<GatewayKind>,
        authority_mode: AuthorityMode,
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        trace_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            protocol_version: RUNTIME_PROTOCOL_VERSION.to_string(),
            adapter_kind,
            adapter_version: env!("CARGO_PKG_VERSION").to_string(),
            supported_gateways,
            authority_mode,
            session_id: session_id.into(),
            turn_id: None,
            request_id: request_id.into(),
            parent_event_id: None,
            trace_id: trace_id.into(),
            workspace_root: None,
            provenance: ProvenanceRefs::default(),
            policy: RuntimePolicy::default(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterCapabilities {
    pub adapter_kind: AdapterKind,
    pub adapter_version: String,
    pub supported_protocol_versions: Vec<String>,
    pub supported_gateways: Vec<GatewayKind>,
    pub supported_output_modes: Vec<OutputMode>,
    pub automatic_interception: bool,
    pub authority_modes: Vec<AuthorityMode>,
    pub redaction_available: bool,
    pub raw_store_available: bool,
    pub fallback_behavior: String,
}

impl AdapterCapabilities {
    pub fn cli_default() -> Self {
        Self {
            adapter_kind: AdapterKind::Cli,
            adapter_version: env!("CARGO_PKG_VERSION").to_string(),
            supported_protocol_versions: vec![RUNTIME_PROTOCOL_VERSION.to_string()],
            supported_gateways: vec![
                GatewayKind::Tool,
                GatewayKind::Context,
                GatewayKind::Output,
                GatewayKind::State,
            ],
            supported_output_modes: vec![OutputMode::Text, OutputMode::Json, OutputMode::Jsonl],
            automatic_interception: false,
            authority_modes: vec![
                AuthorityMode::ObserveOnly,
                AuthorityMode::Suggest,
                AuthorityMode::ExecuteWithRuntimeAuthority,
                AuthorityMode::RequiresUserConfirmation,
            ],
            redaction_available: true,
            raw_store_available: true,
            fallback_behavior: "neutral_cli_or_fail_closed".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NegotiationRequest {
    pub required_protocol_version: String,
    pub required_gateways: Vec<GatewayKind>,
    pub required_output_mode: OutputMode,
    pub minimum_authority_mode: AuthorityMode,
    pub require_redaction: bool,
    pub require_raw_store: bool,
    pub adapter_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NegotiationResult {
    pub accepted: bool,
    pub fallback_reason: Option<FallbackReason>,
    pub message: String,
}

pub fn negotiate(cap: &AdapterCapabilities, req: &NegotiationRequest) -> NegotiationResult {
    if !req.adapter_enabled {
        return rejected(
            FallbackReason::AdapterDisabled,
            "adapter disabled; use neutral TFY CLI/core behavior",
        );
    }
    if !cap
        .supported_protocol_versions
        .iter()
        .any(|v| v == &req.required_protocol_version)
    {
        return rejected(
            FallbackReason::UnsupportedProtocolVersion,
            "unsupported runtime protocol version",
        );
    }
    if req
        .required_gateways
        .iter()
        .any(|g| !cap.supported_gateways.contains(g))
    {
        return rejected(
            FallbackReason::UnsupportedGateway,
            "adapter does not support required gateway",
        );
    }
    if !cap
        .supported_output_modes
        .contains(&req.required_output_mode)
    {
        return rejected(
            FallbackReason::UnsupportedOutputMode,
            "adapter does not support required output mode",
        );
    }
    if req.require_redaction && !cap.redaction_available {
        return rejected(
            FallbackReason::RedactionUnavailable,
            "adapter cannot provide public redaction",
        );
    }
    if req.require_raw_store && !cap.raw_store_available {
        return rejected(
            FallbackReason::MissingRawStore,
            "adapter lacks raw store fallback",
        );
    }
    if !cap
        .authority_modes
        .iter()
        .any(|mode| mode >= &req.minimum_authority_mode)
    {
        return rejected(
            FallbackReason::InsufficientAuthority,
            "adapter lacks required authority mode",
        );
    }
    NegotiationResult {
        accepted: true,
        fallback_reason: None,
        message: "accepted".into(),
    }
}

fn rejected(reason: FallbackReason, message: &str) -> NegotiationResult {
    NegotiationResult {
        accepted: false,
        fallback_reason: Some(reason),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GatewayRequest {
    ToolCommand {
        command: Vec<String>,
    },
    Context {
        path: String,
        scope: String,
        compactness: String,
        diagnostics: Option<String>,
    },
    Output {
        restore_payload: serde_json::Value,
        apply: bool,
    },
    StateProjection {
        ledger_ref: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GatewayResponse {
    ToolCommand {
        command: String,
        exit_code: i32,
        risk: String,
        #[serde(default)]
        command_family: String,
        summary: String,
        model_text: String,
        rendering_kind: String,
        raw_ref: String,
        evidence: Vec<String>,
    },
    Context {
        action: String,
        reason: String,
        compact_context: Option<serde_json::Value>,
        full_context: Option<serde_json::Value>,
        context_ref: String,
    },
    Output {
        restored_code: String,
        preview_diff: String,
        validation_status: ValidationStatus,
        applied: bool,
        patch_ref: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        applied_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed_range: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_hash: Option<String>,
    },
    StateProjection {
        projection: Box<StateProjection>,
    },
    Fallback {
        reason: FallbackReason,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GatewayEvent {
    ToolCommandCompleted {
        command: String,
        exit_code: i32,
        risk: String,
        #[serde(default)]
        command_family: String,
        raw_ref: String,
        #[serde(default)]
        raw_bytes: usize,
        #[serde(default)]
        model_bytes: usize,
        #[serde(default)]
        raw_chars: usize,
        #[serde(default)]
        summary_chars: usize,
        #[serde(default)]
        model_chars: usize,
        #[serde(default)]
        savings_pct: f64,
        #[serde(default)]
        negative_savings_avoided: bool,
        #[serde(default)]
        rendering_kind: String,
    },
    ContextSelected {
        context_ref: String,
        action: String,
        reason: String,
    },
    OutputValidated {
        patch_ref: String,
        validation_status: ValidationStatus,
        applied: bool,
    },
    StateProjected {
        ledger_ref: String,
        authoritative: bool,
    },
    Fallback {
        reason: FallbackReason,
        message: String,
    },
    Validation {
        status: ValidationStatus,
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StateProjection {
    pub goal: Option<String>,
    pub constraints: Vec<String>,
    pub decisions: Vec<String>,
    pub changed_files: Vec<String>,
    pub tool_evidence: Vec<String>,
    pub context_refs: Vec<String>,
    pub blockers: Vec<String>,
    pub verification: Vec<String>,
    pub next_actions: Vec<String>,
    pub source_event_ids: Vec<String>,
    pub provenance: ProvenanceRefs,
    pub authoritative: bool,
    pub fallback_reason: Option<FallbackReason>,
}

pub fn append_event(path: impl AsRef<Path>, event: &RuntimeEnvelope<GatewayEvent>) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    writeln!(file, "{}", serde_json::to_string(event)?)?;
    Ok(())
}

pub fn load_events(path: impl AsRef<Path>) -> Result<Vec<RuntimeEnvelope<GatewayEvent>>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: RuntimeEnvelope<GatewayEvent> = serde_json::from_str(line)
            .map_err(|e| anyhow::anyhow!("invalid ledger line {}: {}", idx + 1, e))?;
        events.push(event);
    }
    Ok(events)
}

pub fn project_state(events: &[RuntimeEnvelope<GatewayEvent>]) -> StateProjection {
    let mut projection = StateProjection::default();
    for event in events {
        projection.source_event_ids.push(event.request_id.clone());
        projection
            .provenance
            .raw_refs
            .extend(event.provenance.raw_refs.clone());
        projection
            .provenance
            .context_refs
            .extend(event.provenance.context_refs.clone());
        projection
            .provenance
            .patch_refs
            .extend(event.provenance.patch_refs.clone());
        projection
            .provenance
            .ledger_refs
            .extend(event.provenance.ledger_refs.clone());
        match &event.payload {
            GatewayEvent::ToolCommandCompleted {
                command,
                exit_code,
                risk,
                raw_ref,
                raw_bytes,
                model_bytes,
                raw_chars,
                model_chars,
                savings_pct,
                rendering_kind,
                ..
            } => {
                let raw_size = if *raw_bytes == 0 {
                    *raw_chars
                } else {
                    *raw_bytes
                };
                let model_size = if *model_bytes == 0 {
                    *model_chars
                } else {
                    *model_bytes
                };
                projection.tool_evidence.push(format!(
                    "{} exit={} risk={} raw_ref={} rendering={} raw_bytes={} model_bytes={} savings_pct={:.2}",
                    command, exit_code, risk, raw_ref, rendering_kind, raw_size, model_size, savings_pct
                ));
                if *exit_code == 0 {
                    projection.verification.push(format!("passed: {command}"));
                } else {
                    projection.blockers.push(format!("failed: {command}"));
                }
            }
            GatewayEvent::ContextSelected {
                context_ref,
                action,
                reason,
            } => {
                projection.context_refs.push(context_ref.clone());
                projection
                    .decisions
                    .push(format!("context {action}: {reason}"));
            }
            GatewayEvent::OutputValidated {
                patch_ref,
                validation_status,
                applied,
            } => {
                projection
                    .changed_files
                    .push(format!("patch_ref={patch_ref} applied={applied}"));
                projection
                    .decisions
                    .push(format!("output validation: {validation_status:?}"));
            }
            GatewayEvent::Fallback { reason, message } => {
                projection
                    .blockers
                    .push(format!("fallback {reason:?}: {message}"));
            }
            GatewayEvent::Validation { status, message } => {
                projection
                    .verification
                    .push(format!("{status:?}: {message}"));
            }
            GatewayEvent::StateProjected { .. } | GatewayEvent::Error { .. } => {}
        }
    }
    projection.provenance.source_event_ids = projection.source_event_ids.clone();
    let has_refs = |event: &RuntimeEnvelope<GatewayEvent>| {
        !event.provenance.raw_refs.is_empty()
            || !event.provenance.context_refs.is_empty()
            || !event.provenance.patch_refs.is_empty()
            || !event.provenance.ledger_refs.is_empty()
    };
    let event_contributes_state = |event: &RuntimeEnvelope<GatewayEvent>| {
        !matches!(
            event.payload,
            GatewayEvent::Validation { .. } | GatewayEvent::StateProjected { .. }
        )
    };
    let contributing: Vec<_> = events
        .iter()
        .filter(|event| event_contributes_state(event))
        .collect();
    let all_contributing_events_are_valid = !contributing.is_empty()
        && contributing.iter().all(|event| {
            !event.request_id.is_empty()
                && has_refs(event)
                && event.provenance.validation_status == Some(ValidationStatus::Valid)
        });
    projection.authoritative = all_contributing_events_are_valid;
    projection.provenance.validation_status = Some(if all_contributing_events_are_valid {
        ValidationStatus::Valid
    } else {
        ValidationStatus::NonAuthoritative
    });
    if !all_contributing_events_are_valid {
        projection.fallback_reason = Some(FallbackReason::MissingProvenance);
    }
    projection
}

pub fn event_envelope(
    payload: GatewayEvent,
    session_id: &str,
    request_id: &str,
    trace_id: &str,
    parent_event_id: Option<String>,
    provenance: ProvenanceRefs,
) -> RuntimeEnvelope<GatewayEvent> {
    let mut envelope = RuntimeEnvelope::new(
        AdapterKind::Cli,
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

pub fn response_envelope(
    payload: GatewayResponse,
    session_id: &str,
    request_id: &str,
    trace_id: &str,
    parent_event_id: Option<String>,
    provenance: ProvenanceRefs,
) -> RuntimeEnvelope<GatewayResponse> {
    let mut envelope = RuntimeEnvelope::new(
        AdapterKind::Cli,
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

pub fn validate_envelope<T>(envelope: &RuntimeEnvelope<T>) -> Result<()> {
    if envelope.protocol_version != RUNTIME_PROTOCOL_VERSION {
        bail!(
            "unsupported runtime protocol version: {}",
            envelope.protocol_version
        );
    }
    if envelope.session_id.is_empty()
        || envelope.request_id.is_empty()
        || envelope.trace_id.is_empty()
    {
        bail!("runtime envelope requires session_id, request_id, and trace_id");
    }
    if envelope.supported_gateways.is_empty() {
        bail!("runtime envelope requires at least one supported gateway");
    }
    Ok(())
}
