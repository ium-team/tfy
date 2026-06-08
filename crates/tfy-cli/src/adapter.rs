use crate::gateways::execute_structured_tool_gateway;
use crate::util::print_json;
use anyhow::{bail, Result};
use clap::Subcommand;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tfy_core::classify_command_family;
use tfy_runtime::{load_events, GatewayEvent};

fn estimate_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

#[derive(Subcommand)]
pub(crate) enum AdapterCmd {
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

pub(crate) fn execute_adapter(cmd: AdapterCmd) -> Result<()> {
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

pub(crate) fn execute_adapter_install(
    target: &str,
    dry_run: bool,
    output: Option<PathBuf>,
) -> Result<()> {
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

#[derive(Default, serde::Serialize, Clone)]
pub(crate) struct FamilySavings {
    family: String,
    commands: usize,
    raw_bytes: usize,
    model_bytes: usize,
    saved_bytes: isize,
    estimated_saved_tokens: isize,
}

#[derive(Default, serde::Serialize)]
pub(crate) struct AdapterReport {
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
    family_counts: BTreeMap<String, usize>,
    families_by_saved_tokens: Vec<FamilySavings>,
    raw_refs: Vec<String>,
}

pub(crate) fn build_adapter_report(ledger: PathBuf, session: &str) -> Result<AdapterReport> {
    let events = load_events(ledger)?;
    let mut report = AdapterReport {
        session: session.into(),
        ..Default::default()
    };
    for event in events.iter().filter(|event| event.session_id == session) {
        if let GatewayEvent::ToolCommandCompleted {
            command,
            exit_code,
            command_family,
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
            let family = if command_family.is_empty() {
                classify_command_family(command)
            } else {
                command_family.clone()
            };
            *report.family_counts.entry(family).or_insert(0) += 1;
            report.raw_refs.push(raw_ref.clone());
        }
    }
    report.families_by_saved_tokens = build_family_savings(&events, session);
    report.saved_bytes = saved_bytes(report.raw_bytes, report.model_bytes);
    report.net_savings_ratio = if report.raw_bytes == 0 {
        0.0
    } else {
        report.saved_bytes as f64 / report.raw_bytes as f64
    };
    report.estimated_raw_tokens = estimate_tokens(report.raw_bytes);
    report.estimated_model_tokens = estimate_tokens(report.model_bytes);
    report.estimated_saved_tokens =
        saved_bytes(report.estimated_raw_tokens, report.estimated_model_tokens);
    report.savings_pct = report.net_savings_ratio * 100.0;
    Ok(report)
}

fn build_family_savings(
    events: &[tfy_runtime::RuntimeEnvelope<GatewayEvent>],
    session: &str,
) -> Vec<FamilySavings> {
    let mut families: BTreeMap<String, FamilySavings> = BTreeMap::new();
    for event in events.iter().filter(|event| event.session_id == session) {
        if let GatewayEvent::ToolCommandCompleted {
            command,
            command_family,
            raw_bytes,
            model_bytes,
            raw_chars,
            summary_chars,
            model_chars,
            ..
        } = &event.payload
        {
            let family = if command_family.is_empty() {
                classify_command_family(command)
            } else {
                command_family.clone()
            };
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
            let entry = families
                .entry(family.clone())
                .or_insert_with(|| FamilySavings {
                    family,
                    ..Default::default()
                });
            entry.commands += 1;
            entry.raw_bytes += raw_size;
            entry.model_bytes += model_size;
        }
    }
    let mut out = families
        .into_values()
        .map(|mut f| {
            f.saved_bytes = saved_bytes(f.raw_bytes, f.model_bytes);
            f.estimated_saved_tokens =
                saved_bytes(estimate_tokens(f.raw_bytes), estimate_tokens(f.model_bytes));
            f
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.saved_bytes
            .cmp(&a.saved_bytes)
            .then_with(|| a.family.cmp(&b.family))
    });
    out
}

fn saved_bytes(raw: usize, model: usize) -> isize {
    raw.saturating_sub(model) as isize
}

pub(crate) fn execute_adapter_report(ledger: PathBuf, session: &str, json: bool) -> Result<()> {
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
        println!("family_counts={:?}", report.family_counts);
        println!(
            "families_by_saved_tokens={}",
            serde_json::to_string(&report.families_by_saved_tokens)?
        );
        if !report.raw_refs.is_empty() {
            println!("raw_refs={}", report.raw_refs.join(","));
        }
        Ok(())
    }
}
