use crate::util::{print_json, read_payload, stable_id};
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tfy_core::*;
use tfy_runtime::*;

struct EnvelopeMeta<'a> {
    session_id: &'a str,
    request_id: &'a str,
    trace_id: &'a str,
    parent_event_id: Option<String>,
    provenance: ProvenanceRefs,
    adapter_kind: AdapterKind,
    origin: Origin,
    route: RouteEvidence,
}

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
    execute_structured_tool_gateway_with_origin(
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
        AdapterKind::Cli,
        Origin::human_cli(),
        RouteEvidence::cli_gateway(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_structured_tool_gateway_with_origin(
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
    adapter_kind: AdapterKind,
    origin: Origin,
    route: RouteEvidence,
) -> Result<()> {
    let (mut event, mut response, exit_code) = tool_gateway_envelopes(
        command,
        raw_dir,
        max_summary_bytes,
        session_id,
        request_id,
        trace_id,
        parent_event_id,
        adapter_kind,
        origin,
        route,
    )?;
    apply_repeated_output_elision(&ledger, &mut event, &mut response);
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
    origin: Origin,
    route: RouteEvidence,
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
    let raw_bytes = summary.raw_chars;
    let model_bytes = summary.model_text.len();
    let route = route.with_sizes(raw_bytes, model_bytes);
    let provenance = ProvenanceRefs {
        raw_refs: vec![summary.raw_ref.clone()],
        route_refs: route.route_id.clone().into_iter().collect(),
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
            strategy_kind: summary.strategy_kind.clone(),
            human_auto_safe: summary.human_auto_safe,
            agent_safe: summary.agent_safe,
            interactive_risk: summary.interactive_risk.clone(),
            summary: summary.summary.clone(),
            model_text: summary.model_text.clone(),
            rendering_kind: summary.rendering_kind.clone(),
            output_sha256: summary.output_sha256.clone(),
            raw_ref: summary.raw_ref.clone(),
            evidence: summary.evidence.clone(),
        },
        EnvelopeMeta {
            session_id: &session_id,
            request_id: &request_id,
            trace_id: &trace_id,
            parent_event_id: parent_event_id.clone(),
            provenance: provenance.clone(),
            adapter_kind: adapter_kind.clone(),
            origin: origin.clone(),
            route: route.clone(),
        },
    );
    let event = gateway_event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: summary.command,
            exit_code: summary.exit_code,
            risk: summary.risk,
            command_family: summary.command_family,
            strategy_kind: summary.strategy_kind,
            human_auto_safe: summary.human_auto_safe,
            agent_safe: summary.agent_safe,
            interactive_risk: summary.interactive_risk,
            raw_ref: summary.raw_ref,
            raw_bytes,
            model_bytes,
            raw_chars: summary.raw_chars,
            summary_chars: summary.summary_chars,
            model_chars: model_bytes,
            savings_pct: summary.savings_pct,
            negative_savings_avoided: summary.rendering_kind == "pass_through"
                && summary.raw_chars <= summary.summary_chars,
            rendering_kind: summary.rendering_kind,
            output_sha256: summary.output_sha256,
        },
        EnvelopeMeta {
            session_id: &session_id,
            request_id: &request_id,
            trace_id: &trace_id,
            parent_event_id,
            provenance,
            adapter_kind,
            origin,
            route,
        },
    );
    let exit_code = match &response.payload {
        GatewayResponse::ToolCommand { exit_code, .. } => *exit_code,
        _ => 1,
    };
    Ok((event, response, exit_code))
}

pub(crate) fn apply_repeated_output_elision(
    ledger: &Path,
    event: &mut RuntimeEnvelope<GatewayEvent>,
    response: &mut RuntimeEnvelope<GatewayResponse>,
) {
    let GatewayEvent::ToolCommandCompleted {
        command,
        exit_code,
        command_family,
        raw_ref,
        raw_bytes,
        model_bytes,
        raw_chars,
        summary_chars,
        model_chars,
        savings_pct,
        rendering_kind,
        output_sha256,
        ..
    } = &mut event.payload
    else {
        return;
    };
    if output_sha256.is_empty() || *raw_bytes == 0 && *raw_chars == 0 {
        return;
    }
    let Ok(events) = load_events(ledger) else {
        return;
    };
    let raw_size = if *raw_bytes == 0 {
        *raw_chars
    } else {
        *raw_bytes
    };
    let Some(previous_raw_ref) = events.iter().rev().find_map(|previous| {
        if previous.session_id != event.session_id {
            return None;
        }
        match &previous.payload {
            GatewayEvent::ToolCommandCompleted {
                command: previous_command,
                exit_code: previous_exit_code,
                raw_ref: previous_raw_ref,
                raw_bytes: previous_raw_bytes,
                raw_chars: previous_raw_chars,
                output_sha256: previous_hash,
                ..
            } if previous_command == command
                && previous_exit_code == exit_code
                && previous_hash == output_sha256
                && previous_raw_ref != raw_ref
                && (if *previous_raw_bytes == 0 {
                    *previous_raw_chars
                } else {
                    *previous_raw_bytes
                }) == raw_size =>
            {
                Some(previous_raw_ref.clone())
            }
            _ => None,
        }
    }) else {
        return;
    };
    let repeated_text = format!(
        "[tfy: repeated unchanged command output elided; command_family={command_family} previous_raw_ref={previous_raw_ref} raw_ref={raw_ref}]\n"
    );
    if repeated_text.len() >= raw_size {
        return;
    }
    let repeated_model_bytes = repeated_text.len();
    *rendering_kind = "repeat_elided".into();
    *model_bytes = repeated_model_bytes;
    *model_chars = repeated_model_bytes;
    *summary_chars = repeated_model_bytes;
    *savings_pct = if raw_size == 0 {
        0.0
    } else {
        ((raw_size as f64 - repeated_model_bytes as f64) / raw_size as f64 * 10000.0).round()
            / 100.0
    };
    event.route = event
        .route
        .clone()
        .with_sizes(raw_size, repeated_model_bytes);
    response.route = response
        .route
        .clone()
        .with_sizes(raw_size, repeated_model_bytes);
    if let GatewayResponse::ToolCommand {
        summary,
        model_text,
        rendering_kind,
        evidence,
        ..
    } = &mut response.payload
    {
        *summary = repeated_text.clone();
        *model_text = repeated_text;
        *rendering_kind = "repeat_elided".into();
        evidence.push(format!("unchanged repeat of raw_ref={previous_raw_ref}"));
    }
}

fn gateway_event_envelope(
    payload: GatewayEvent,
    meta: EnvelopeMeta<'_>,
) -> RuntimeEnvelope<GatewayEvent> {
    let mut envelope = RuntimeEnvelope::new(
        meta.adapter_kind,
        vec![
            GatewayKind::Tool,
            GatewayKind::Context,
            GatewayKind::Output,
            GatewayKind::State,
        ],
        AuthorityMode::ExecuteWithRuntimeAuthority,
        meta.session_id,
        meta.request_id,
        meta.trace_id,
        payload,
    );
    envelope.parent_event_id = meta.parent_event_id;
    envelope.provenance = meta.provenance;
    envelope.origin = meta.origin;
    envelope.route = meta.route;
    envelope
}

fn gateway_response_envelope(
    payload: GatewayResponse,
    meta: EnvelopeMeta<'_>,
) -> RuntimeEnvelope<GatewayResponse> {
    let mut envelope = RuntimeEnvelope::new(
        meta.adapter_kind,
        vec![
            GatewayKind::Tool,
            GatewayKind::Context,
            GatewayKind::Output,
            GatewayKind::State,
        ],
        AuthorityMode::ExecuteWithRuntimeAuthority,
        meta.session_id,
        meta.request_id,
        meta.trace_id,
        payload,
    );
    envelope.parent_event_id = meta.parent_event_id;
    envelope.provenance = meta.provenance;
    envelope.origin = meta.origin;
    envelope.route = meta.route;
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
    let context_ref = exp
        .apply_proof
        .as_ref()
        .map(|proof| proof.context_ref.clone())
        .unwrap_or_else(|| {
            stable_id(&format!(
                "{}:{}:{}:{}:{}",
                exp.scope.path,
                exp.scope.start_line,
                exp.scope.end_line,
                compactness,
                exp.compact_code
            ))
        });
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
    context_proof: Option<PathBuf>,
    session_id: String,
    request_id: Option<String>,
    trace_id: Option<String>,
    parent_event_id: Option<String>,
) -> Result<()> {
    let request_id = request_id.unwrap_or_else(|| stable_id("output"));
    let trace_id = trace_id.unwrap_or_else(|| request_id.clone());
    let text = read_payload(payload)?;
    let restore_body: RestorePayload = serde_json::from_str(&text)?;
    if apply {
        let proof_override = context_proof
            .map(std::fs::read_to_string)
            .transpose()?
            .map(|text| serde_json::from_str::<ApplyProof>(&text))
            .transpose()?;
        let authority_proof = match (restore_body.apply_proof.as_ref(), proof_override.as_ref()) {
            (Some(proof), None) | (None, Some(proof)) => Some(proof.clone()),
            _ => None,
        };
        if parent_event_id.is_some()
            && proof_override.is_none()
            && restore_body.apply_proof.is_none()
        {
            bail!("parent event id alone is not authoritative for output-gateway apply");
        }
        let applied = apply_restored_payload(restore_body, proof_override)?;
        let preview_diff = format!(
            "--- original-scope\n+++ applied-scope\n@@ byte {}..{}\n{}",
            applied.byte_start, applied.byte_end, applied.restored_code
        );
        let provenance = ProvenanceRefs {
            context_refs: vec![applied.context_ref.clone()],
            patch_refs: vec![applied.patch_ref.clone()],
            apply_authority: authority_proof.map(|proof| ApplyAuthorityEvidence {
                context_ref: Some(proof.context_ref),
                base_content_hash: Some(proof.source_sha256),
                proof_id: Some(stable_id(&format!(
                    "{}:{}:{}:{}",
                    proof.path, proof.scope_id, proof.byte_start, proof.byte_end
                ))),
                proof_hash: Some(proof.compact_code_sha256),
                byte_range: Some(format!("{}..{}", proof.byte_start, proof.byte_end)),
                unique_anchors: vec![proof.scope_id],
                workspace_scope: Some(proof.path),
                validate_succeeded: true,
                parent_event_id_only: false,
            }),
            validation_status: Some(ValidationStatus::Valid),
            ..Default::default()
        };
        let response = response_envelope(
            GatewayResponse::Output {
                restored_code: applied.restored_code,
                preview_diff,
                validation_status: ValidationStatus::Valid,
                applied: true,
                patch_ref: applied.patch_ref,
                applied_path: Some(applied.path),
                changed_range: Some(serde_json::json!({
                    "scope_id": applied.scope_id,
                    "byte_start": applied.byte_start,
                    "byte_end": applied.byte_end
                })),
                before_hash: Some(applied.before_sha256),
                after_hash: Some(applied.after_sha256),
            },
            &session_id,
            &request_id,
            &trace_id,
            parent_event_id,
            provenance,
        );
        return print_json(&response);
    }
    if context_proof.is_some() {
        bail!("--context-proof is only valid with --apply");
    }
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
        apply_authority: Some(ApplyAuthorityEvidence {
            validate_succeeded: false,
            parent_event_id_only: parent_event_id.is_some(),
            ..Default::default()
        }),
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
            applied_path: None,
            changed_range: None,
            before_hash: None,
            after_hash: None,
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
