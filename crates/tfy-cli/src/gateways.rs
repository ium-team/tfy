use crate::util::{print_json, read_payload, stable_id};
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
use tfy_core::*;
use tfy_runtime::*;

pub(crate) fn execute_plain_tool_gateway(
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
pub(crate) fn execute_structured_tool_gateway(
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
pub(crate) fn tool_gateway_envelopes(
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
    let provenance = ProvenanceRefs {
        raw_refs: vec![summary.raw_ref.clone()],
        source_event_ids: vec![request_id.clone()],
        validation_status: Some(ValidationStatus::Valid),
        ..Default::default()
    };
    let response = gateway_response_envelope(
        GatewayResponse::ToolCommand {
            command: summary.command.clone(),
            exit_code: summary.exit_code,
            risk: summary.risk.clone(),
            command_family: summary.command_family.clone(),
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
    let event = gateway_event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: summary.command,
            exit_code: summary.exit_code,
            risk: summary.risk,
            command_family: summary.command_family,
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

pub(crate) fn execute_context_gateway(
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

pub(crate) fn execute_output_gateway(
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

pub(crate) fn context_decision_from_value(v: &serde_json::Value) -> Result<ContextDecision> {
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
