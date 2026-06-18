use anyhow::Result;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tfy_runtime::{GatewayEvent, GatewayResponse, OriginKind, RuntimeEnvelope};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuidanceMode {
    Human,
    Agent,
}

#[derive(Debug, Clone)]
pub(crate) struct CustomGuidance {
    pub(crate) mode: GuidanceMode,
    session_id: String,
    request_id: String,
    command: String,
    pub(crate) argv0: String,
    command_family: String,
    strategy_kind: String,
    rendering_kind: String,
    rule_id: Option<String>,
    source_kind: String,
    reason: String,
    raw_ref: String,
    ledger_event_id: String,
}

pub(crate) fn custom_guidance_for(
    event: &RuntimeEnvelope<GatewayEvent>,
    response: &RuntimeEnvelope<GatewayResponse>,
    original_command: &[String],
) -> Option<CustomGuidance> {
    let GatewayResponse::ToolCommand {
        command,
        command_family,
        strategy_kind,
        rendering_kind,
        rule_id,
        strategy_source_kind,
        raw_ref,
        ..
    } = &response.payload
    else {
        return None;
    };
    if strategy_kind != "generic" || rendering_kind != "summary" {
        return None;
    }
    let mode = match event.origin.kind {
        OriginKind::HumanCli if event.origin.intercepted => GuidanceMode::Human,
        OriginKind::AgentRuntime if event.origin.intercepted => GuidanceMode::Agent,
        _ => return None,
    };
    Some(CustomGuidance {
        mode,
        session_id: event.session_id.clone(),
        request_id: event.request_id.clone(),
        command: command.clone(),
        argv0: original_command.first().cloned().unwrap_or_default(),
        command_family: command_family.clone(),
        strategy_kind: strategy_kind.clone(),
        rendering_kind: rendering_kind.clone(),
        rule_id: rule_id.clone(),
        source_kind: strategy_source_kind.clone(),
        reason: "generic_summary_without_trusted_builtin_or_custom_rule".into(),
        raw_ref: raw_ref.clone(),
        ledger_event_id: event.request_id.clone(),
    })
}

pub(crate) fn record_custom_guidance(guidance: &CustomGuidance) -> Result<()> {
    let path = guidance_path(guidance.mode);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let ts_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path)?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&json!({
            "schema_version": 1,
            "session": guidance.session_id,
            "request_id": guidance.request_id,
            "ts_unix_ms": ts_unix_ms,
            "command": guidance.command,
            "argv0": guidance.argv0,
            "command_family": guidance.command_family,
            "strategy_kind": guidance.strategy_kind,
            "rendering_kind": guidance.rendering_kind,
            "rule_id": guidance.rule_id,
            "source_kind": guidance.source_kind,
            "reason": guidance.reason,
            "raw_ref": guidance.raw_ref,
            "ledger_event_id": guidance.ledger_event_id,
        }))?
    )?;
    Ok(())
}

fn guidance_path(mode: GuidanceMode) -> PathBuf {
    match mode {
        GuidanceMode::Human => std::env::var_os("TFY_HUMAN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tfy/human/custom-guidance.jsonl"),
        GuidanceMode::Agent => PathBuf::from(".tfy/agent/custom-guidance.jsonl"),
    }
}

pub(crate) fn print_human_custom_guidance(guidance: &CustomGuidance) {
    print!(
        "\n[tfy] This command used generic summarization. Run `tfy custom` to add a trusted custom rule for `{}`; details saved in .tfy/human/custom-guidance.jsonl.\n",
        guidance.argv0
    );
}
