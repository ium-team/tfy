use crate::gateways::execute_structured_tool_gateway_with_origin;
use crate::util::print_json;
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tfy_runtime::{
    AdapterKind, Origin, OriginHost, OriginInvocation, RouteClaimTier, RouteEvidence,
    RouteIngressKind,
};

#[derive(Subcommand)]
pub(crate) enum AgentCmd {
    /// Print agent-runtime wrapper capabilities and support boundaries.
    Capabilities,
    /// Print or create an explicit AI-agent command wrapper. Does not mutate human shell config.
    Install {
        #[arg(long, default_value = "generic")]
        host: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Report agent-session custom summary guidance recorded without polluting model-visible output.
    Report {
        #[arg(long, default_value = "agent-session")]
        session: String,
        #[arg(long, default_value = ".tfy/agent/custom-guidance.jsonl")]
        guidance: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Run an AI-agent-originated command through TFY with explicit origin provenance.
    Run {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long, default_value = ".tfy/agent/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "agent-session")]
        session: String,
        #[arg(long, default_value = "generic")]
        host: String,
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
}

pub(crate) fn execute_agent(cmd: AgentCmd) -> Result<()> {
    match cmd {
        AgentCmd::Capabilities => print_json(&serde_json::json!({
            "adapter_version": env!("CARGO_PKG_VERSION"),
            "target": "agent-runtime-wrapper",
            "automatic_interception": "configured_ai_agent_wrapper_only",
            "ordinary_terminal_interception": false,
            "mutates_shell_startup_files_by_default": false,
            "origin_contract": {
                "kind": "agent_runtime",
                "invocation": "wrapper",
                "intercepted": true,
                "user_shell_mutated": false
            },
            "not_claimed": ["private_codex_hook", "provider_prompt_gateway", "universal_shell_interception", "editor_integration"]
        })),
        AgentCmd::Install {
            host,
            dry_run,
            output,
        } => execute_agent_install(&host, dry_run, output),
        AgentCmd::Report {
            session,
            guidance,
            json,
        } => execute_agent_report(&session, &guidance, json),
        AgentCmd::Run {
            raw_dir,
            max_summary_bytes,
            ledger,
            session,
            host,
            request_id,
            trace_id,
            parent_event_id,
            json,
            jsonl,
            command,
        } => execute_structured_tool_gateway_with_origin(
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
            AdapterKind::Cli,
            Origin::agent_runtime(parse_host(&host), OriginInvocation::Wrapper),
            RouteEvidence {
                ingress: RouteIngressKind::AgentWrapper,
                host: parse_host(&host),
                claim_tier: RouteClaimTier::RouteEvidenceRecorded,
                official_docs_backed: true,
                kill_switch_available: true,
                uninstall_available: true,
                ..RouteEvidence::cli_gateway()
            },
        ),
    }
}

fn execute_agent_install(host: &str, dry_run: bool, output: Option<PathBuf>) -> Result<()> {
    let script = format!(
        r#"#!/usr/bin/env sh
# TFY AI-agent command wrapper. Configure an AI agent runtime to call this wrapper.
# This file is not installed into .zshrc/.bashrc/profile by TFY.
exec tfy agent run --host {host} --session "${{TFY_SESSION_ID:-agent-session}}" -- "$@"
"#
    );
    if dry_run || output.is_none() {
        let path = output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "./tfy-agent-wrapper".into());
        println!("TFY agent wrapper install dry-run");
        println!("target=agent-runtime-wrapper status=supported");
        println!("host={host}");
        println!("would_write={path}");
        println!("ordinary_terminal_interception=false");
        println!("mutates_shell_startup_files_by_default=false");
        println!("configure only your AI agent runtime command wrapper to call: {path} -- <ordinary command>");
        println!("script:\n{script}");
        return Ok(());
    }
    let Some(output) = output else {
        bail!("agent install requires --output unless --dry-run is used");
    };
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    if output.exists() {
        fs::copy(&output, output.with_extension("bak"))?;
    }
    fs::write(&output, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&output, fs::Permissions::from_mode(0o755))?;
    }
    println!("installed TFY AI-agent wrapper at {}", output.display());
    println!("ordinary terminal startup files were not modified");
    Ok(())
}

fn execute_agent_report(session: &str, guidance: &PathBuf, json: bool) -> Result<()> {
    let entries = read_guidance_entries(guidance, session)?;
    let mut command_counts = BTreeMap::<String, usize>::new();
    let mut reason_counts = BTreeMap::<String, usize>::new();
    for entry in &entries {
        if let Some(command) = entry.get("argv0").and_then(|value| value.as_str()) {
            *command_counts.entry(command.to_string()).or_default() += 1;
        }
        if let Some(reason) = entry.get("reason").and_then(|value| value.as_str()) {
            *reason_counts.entry(reason.to_string()).or_default() += 1;
        }
    }
    if json {
        print_json(&serde_json::json!({
            "schema_version": 1,
            "session": session,
            "guidance_path": guidance,
            "needs_custom_rules": !entries.is_empty(),
            "total": entries.len(),
            "command_counts": command_counts,
            "reason_counts": reason_counts,
            "entries": entries,
        }))?;
        return Ok(());
    }
    println!("TFY agent custom summary report");
    println!("session={session}");
    println!("guidance_path={}", guidance.display());
    if entries.is_empty() {
        println!("needs_custom_rules=false");
        println!("No weak/generic command summaries were recorded for this session.");
        return Ok(());
    }
    println!("needs_custom_rules=true total={}", entries.len());
    for (command, count) in command_counts {
        println!("custom_rule_candidate command={command} count={count}");
    }
    println!(
        "Run `tfy custom` to author trusted command rules for commands that need better summaries."
    );
    Ok(())
}

fn read_guidance_entries(path: &PathBuf, session: &str) -> Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut entries = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("parse {} line {}", path.display(), idx + 1))?;
        if value.get("session").and_then(|session| session.as_str()) == Some(session) {
            entries.push(value);
        }
    }
    Ok(entries)
}

fn parse_host(host: &str) -> OriginHost {
    match host.to_ascii_lowercase().as_str() {
        "codex" => OriginHost::Codex,
        "omx" => OriginHost::Omx,
        "generic" => OriginHost::Generic,
        _ => OriginHost::Unknown,
    }
}
