use crate::gateways::execute_structured_tool_gateway_with_origin;
use crate::util::print_json;
use anyhow::{anyhow, bail, Context, Result};
use clap::Subcommand;
use std::io::{self, Read};
use std::path::PathBuf;
use tfy_runtime::{
    AdapterKind, Origin, OriginHost, OriginInvocation, RouteClaimTier, RouteEvidence,
    RouteIngressKind,
};

#[derive(Subcommand)]
pub(crate) enum HookCmd {
    /// Print host-hook support boundaries. Hooks are optional official-host routers.
    Capabilities,
    /// Print dry-run setup guidance for a future host hook route.
    Install {
        #[arg(long, default_value = "test-shim")]
        target: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Official host hook ingress. PreToolUse rewrites host Bash input; explicit commands execute through the shared Tool Gateway.
    Run {
        #[arg(long, default_value = "test-shim")]
        host: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long, default_value = ".tfy/hook/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "hook-session")]
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
}

pub(crate) fn execute_hook(cmd: HookCmd) -> Result<()> {
    match cmd {
        HookCmd::Capabilities => print_json(&serde_json::json!({
            "adapter_version": env!("CARGO_PKG_VERSION"),
            "default_policy": "official_docs_backed_for_codex_and_claude_code_but_launch_evidence_gated",
            "role": "thin_router_to_shared_tfy_gateways",
            "does_not_implement": ["summarization", "redaction", "restore", "apply", "claim_promotion"],
            "kill_switch": {
                "env": "TFY_HOOK_DISABLE=1",
                "per_host": "planned",
                "uninstall": "required_before_host_writer_support"
            },
            "targets": [
                {"target":"test-shim","status":"supported_for_equivalence_tests","claim_tier":"route_evidence_recorded"},
                {"target":"codex","status":"supported_configured_unverified","claim_tier":"config_written"},
                {"target":"claude-code","status":"supported_configured_unverified","claim_tier":"config_written"},
                {"target":"cursor","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
                {"target":"opencode","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
                {"target":"hermes","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
                {"target":"openclaw","status":"planned_discovery","claim_tier":"planned_discovery"}
            ],
            "not_claimed": ["private_codex_hook", "provider_prompt_gateway", "universal_shell_interception", "editor_auto_integration"]
        })),
        HookCmd::Install { target, dry_run } => execute_hook_install(&target, dry_run),
        HookCmd::Run {
            host,
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
        } => {
            if std::env::var("TFY_HOOK_DISABLE").ok().as_deref() == Some("1") {
                bail!("TFY hook route disabled by TFY_HOOK_DISABLE=1");
            }
            ensure_supported_hook_host(&host)?;
            if command.is_empty() && is_real_hook_host(&host) {
                return print_host_rewrite(&host, raw_dir, ledger, session);
            }
            let command = resolve_hook_command(&host, command)?;
            execute_structured_tool_gateway_with_origin(
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
                AdapterKind::TestHarness,
                Origin::agent_runtime(parse_hook_host(&host), OriginInvocation::OfficialHostHook),
                hook_route_evidence(&host),
            )
        }
    }
}

fn ensure_supported_hook_host(host: &str) -> Result<()> {
    if host.eq_ignore_ascii_case("test-shim") || is_real_hook_host(host) {
        return Ok(());
    }
    bail!("unsupported hook target '{host}'; supported hook run targets are test-shim, codex, and claude-code")
}

fn is_real_hook_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "codex" | "claude-code" | "claude"
    )
}

fn execute_hook_install(target: &str, dry_run: bool) -> Result<()> {
    if !dry_run {
        bail!("hook install is dry-run only until host official docs, kill-switch, uninstall, and e2e evidence exist");
    }
    let status = match target {
        "test-shim" => "supported_for_equivalence_tests",
        "codex" | "claude-code" => "supported_configured_unverified",
        "cursor" | "opencode" | "hermes" | "openclaw" => "planned_official_docs_required",
        other => bail!("unknown hook target '{other}'"),
    };
    println!("TFY hook install dry-run");
    println!("target={target} status={status}");
    println!("boundary=official host hooks only; no private Codex/provider/universal interception");
    println!("kill_switch=TFY_HOOK_DISABLE=1");
    println!("route=host hook event -> tfy hook run --host {target} -> shared Tool Gateway");
    if matches!(target, "codex" | "claude-code") {
        println!("launch_supported=false until real host evidence proves route-bound raw/ledger/no-negative/positive-savings");
    }
    Ok(())
}

fn hook_route_evidence(host: &str) -> RouteEvidence {
    let parsed = parse_hook_host(host);
    let supported_test_shim = host.eq_ignore_ascii_case("test-shim");
    let supported_official_host = matches!(
        host.to_ascii_lowercase().as_str(),
        "codex" | "claude-code" | "claude"
    );
    RouteEvidence {
        ingress: RouteIngressKind::HostHook,
        host: parsed,
        claim_tier: if supported_test_shim {
            RouteClaimTier::RouteEvidenceRecorded
        } else if supported_official_host {
            RouteClaimTier::ConfigWritten
        } else {
            RouteClaimTier::PlannedDiscovery
        },
        official_docs_backed: supported_test_shim || supported_official_host,
        kill_switch_available: true,
        uninstall_available: supported_test_shim || supported_official_host,
        ..RouteEvidence::cli_gateway()
    }
}

fn parse_hook_host(host: &str) -> OriginHost {
    match host.to_ascii_lowercase().as_str() {
        "codex" => OriginHost::Codex,
        "claude-code" | "claude" => OriginHost::ClaudeCode,
        "omx" => OriginHost::Omx,
        "test-shim" | "generic" => OriginHost::Generic,
        _ => OriginHost::Unknown,
    }
}

fn resolve_hook_command(host: &str, command: Vec<String>) -> Result<Vec<String>> {
    if !command.is_empty() {
        return Ok(command);
    }
    if host.eq_ignore_ascii_case("test-shim") {
        bail!("hook run requires a command after -- for test-shim");
    }
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .context("read host hook JSON from stdin")?;
    let value: serde_json::Value = serde_json::from_str(&stdin).context(
        "parse host hook JSON from stdin; unsupported or malformed hook payload fails closed",
    )?;
    validate_pre_tool_use_shell_payload(&value)?;
    extract_hook_command(&value)
        .ok_or_else(|| anyhow!("unsupported hook payload: missing shell command field"))
}

fn print_host_rewrite(
    host: &str,
    raw_dir: PathBuf,
    ledger: PathBuf,
    session: String,
) -> Result<()> {
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .context("read host hook JSON from stdin")?;
    let value: serde_json::Value = serde_json::from_str(&stdin).context(
        "parse host hook JSON from stdin; unsupported or malformed hook payload fails closed",
    )?;
    validate_pre_tool_use_shell_payload(&value)?;
    let original = extract_hook_command_text(&value)
        .ok_or_else(|| anyhow!("unsupported hook payload: missing shell command field"))?;
    if command_is_tfy_hook_route(&original) {
        print_json(&serde_json::json!({}))?;
        return Ok(());
    }
    let rewritten = rewrite_command_through_tfy_hook(host, &raw_dir, &ledger, &session, &original)?;
    print_json(&serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": {
                "command": rewritten
            }
        }
    }))
}

fn validate_pre_tool_use_shell_payload(value: &serde_json::Value) -> Result<()> {
    let Some(event) = value
        .pointer("/hook_event_name")
        .or_else(|| value.pointer("/hookEventName"))
        .and_then(serde_json::Value::as_str)
    else {
        bail!("unsupported hook payload: missing hook_event_name=PreToolUse");
    };
    if event != "PreToolUse" {
        bail!("unsupported hook payload: only PreToolUse events are routed through TFY");
    }
    if !hook_payload_targets_shell(value) {
        bail!("unsupported hook payload: only Bash/shell command events are routed through TFY");
    }
    Ok(())
}

fn hook_payload_targets_shell(value: &serde_json::Value) -> bool {
    let candidates = [
        "/tool_name",
        "/tool",
        "/name",
        "/matcher",
        "/tool_input/tool_name",
        "/input/tool_name",
    ];
    for pointer in candidates {
        if let Some(text) = value.pointer(pointer).and_then(serde_json::Value::as_str) {
            let lower = text.to_ascii_lowercase();
            if matches!(lower.as_str(), "bash" | "shell" | "sh") {
                return true;
            }
        }
    }
    false
}

fn extract_hook_command(value: &serde_json::Value) -> Option<Vec<String>> {
    let string_paths = [
        "/command",
        "/tool_input/command",
        "/input/command",
        "/arguments/command",
        "/params/command",
    ];
    for pointer in string_paths {
        if let Some(command) = value.pointer(pointer).and_then(serde_json::Value::as_str) {
            if !command.trim().is_empty() {
                return Some(vec!["sh".into(), "-c".into(), command.into()]);
            }
        }
    }
    let array_paths = ["/argv", "/command_argv", "/tool_input/argv", "/input/argv"];
    for pointer in array_paths {
        if let Some(array) = value.pointer(pointer).and_then(serde_json::Value::as_array) {
            let argv = array
                .iter()
                .map(serde_json::Value::as_str)
                .collect::<Option<Vec<_>>>()?;
            if !argv.is_empty() && argv.iter().all(|part| !part.trim().is_empty()) {
                return Some(argv.into_iter().map(str::to_string).collect());
            }
        }
    }
    None
}

fn extract_hook_command_text(value: &serde_json::Value) -> Option<String> {
    let string_paths = [
        "/command",
        "/tool_input/command",
        "/input/command",
        "/arguments/command",
        "/params/command",
    ];
    string_paths.iter().find_map(|pointer| {
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .filter(|command| !command.trim().is_empty())
            .map(str::to_string)
    })
}

fn rewrite_command_through_tfy_hook(
    host: &str,
    raw_dir: &PathBuf,
    ledger: &PathBuf,
    session: &str,
    original: &str,
) -> Result<String> {
    let exe = std::env::current_exe().context("resolve current tfy executable")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow!("current tfy executable path is not valid UTF-8"))?;
    let raw_dir = absolute_path(raw_dir)?;
    let ledger = absolute_path(ledger)?;
    Ok(format!(
        "{} hook run --host {} --session {} --raw-dir {} --ledger {} -- sh -c {}",
        shell_single_quote(exe),
        shell_single_quote(canonical_hook_host(host)),
        shell_single_quote(session),
        shell_single_quote(&raw_dir.display().to_string()),
        shell_single_quote(&ledger.display().to_string()),
        shell_single_quote(original)
    ))
}

fn absolute_path(path: &PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.clone())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn canonical_hook_host(host: &str) -> &str {
    if host.eq_ignore_ascii_case("claude") {
        "claude-code"
    } else {
        host
    }
}

fn command_is_tfy_hook_route(command: &str) -> bool {
    command.contains(" hook run --host ")
        || command.contains(" hook run --host=")
        || command.starts_with("tfy hook run --host ")
        || command.starts_with("tfy hook run --host=")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
