use crate::gateways::execute_structured_tool_gateway_with_origin;
use crate::util::print_json;
use anyhow::{bail, Result};
use clap::Subcommand;
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
    /// Test-only hook ingress shim. Routes an event command through the shared Tool Gateway.
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
            "default_policy": "disabled_until_official_host_docs_and_e2e_evidence",
            "role": "thin_router_to_shared_tfy_gateways",
            "does_not_implement": ["summarization", "redaction", "restore", "apply", "claim_promotion"],
            "kill_switch": {
                "env": "TFY_HOOK_DISABLE=1",
                "per_host": "planned",
                "uninstall": "required_before_host_writer_support"
            },
            "targets": [
                {"target":"test-shim","status":"supported_for_equivalence_tests","claim_tier":"route_evidence_recorded"},
                {"target":"codex","status":"unsupported_without_public_official_hook","claim_tier":"unsupported"},
                {"target":"cursor","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
                {"target":"opencode","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
                {"target":"claude-code","status":"planned_official_docs_required","claim_tier":"planned_discovery"},
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
            if !host.eq_ignore_ascii_case("test-shim") {
                bail!(
                    "hook run is supported only for test-shim until official host docs, uninstall, kill-switch, and e2e evidence exist for target '{host}'"
                );
            }
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

fn execute_hook_install(target: &str, dry_run: bool) -> Result<()> {
    if !dry_run {
        bail!("hook install is dry-run only until host official docs, kill-switch, uninstall, and e2e evidence exist");
    }
    let status = match target {
        "test-shim" => "supported_for_equivalence_tests",
        "codex" => "unsupported_without_public_official_hook",
        "cursor" | "opencode" | "claude-code" | "hermes" | "openclaw" => {
            "planned_official_docs_required"
        }
        other => bail!("unknown hook target '{other}'"),
    };
    println!("TFY hook install dry-run");
    println!("target={target} status={status}");
    println!("boundary=official host hooks only; no private Codex/provider/universal interception");
    println!("kill_switch=TFY_HOOK_DISABLE=1");
    println!("route=host hook event -> tfy hook run/test shim -> shared Tool Gateway");
    Ok(())
}

fn hook_route_evidence(host: &str) -> RouteEvidence {
    let parsed = parse_hook_host(host);
    let supported_test_shim = host.eq_ignore_ascii_case("test-shim");
    RouteEvidence {
        ingress: RouteIngressKind::HostHook,
        host: parsed,
        claim_tier: if supported_test_shim {
            RouteClaimTier::RouteEvidenceRecorded
        } else {
            RouteClaimTier::PlannedDiscovery
        },
        official_docs_backed: supported_test_shim,
        kill_switch_available: true,
        uninstall_available: supported_test_shim,
        ..RouteEvidence::cli_gateway()
    }
}

fn parse_hook_host(host: &str) -> OriginHost {
    match host.to_ascii_lowercase().as_str() {
        "codex" => OriginHost::Codex,
        "omx" => OriginHost::Omx,
        _ => OriginHost::Generic,
    }
}
