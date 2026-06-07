use crate::util::{print_json, stable_id};
use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tfy_runtime::{load_events, GatewayEvent};

const TFY_CODEX_START: &str = "<!-- TFY:CODEX:START -->";
const TFY_CODEX_END: &str = "<!-- TFY:CODEX:END -->";
const NO_DATA_MESSAGE: &str = "No TFY savings data found yet";
const REQUIRED_MCP_TOOLS: &[&str] = &[
    "tfy_tool_run",
    "tfy_scope_list",
    "tfy_context_get",
    "tfy_output_validate",
    "tfy_output_apply",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeTarget {
    Project,
    Global,
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
    message: Option<String>,
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
    let report = build_doctor_report(cmd.codex);
    if cmd.json {
        print_json(&report)?;
    } else {
        println!("TFY doctor: {}", report.status);
        for d in &report.diagnostics {
            println!("{} {} — {}", status_icon(&d.status), d.name, d.message);
        }
    }
    if report.status == "fail" {
        bail!("tfy doctor found failing local checks");
    }
    Ok(())
}

pub(crate) fn execute_smoke(cmd: SmokeCmd) -> Result<()> {
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
    let listed = child.request(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tfy_scope_list","arguments":{"path":sample.to_str().unwrap(),"query":"add","limit":5}}}))?;
    let listed_json = mcp_content_json(&listed)?;
    let scope_id = listed_json["scopes"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|s| s["id"].as_str())
        .ok_or_else(|| anyhow!("smoke scope not found"))?
        .to_string();
    let context = child.request(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"tfy_context_get","arguments":{"session":"smoke","path":sample.to_str().unwrap(),"scope":scope_id,"compactness":"symbol"}}}))?;
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
    let preview = child.request(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"tfy_output_validate","arguments":{"session":"smoke","restore_payload":restore_payload.clone()}}}))?;
    let preview_json = mcp_content_json(&preview)?;
    if preview_json["applied"] != false {
        bail!("preview unexpectedly applied");
    }
    if fs::read_to_string(&sample)? != before_preview {
        bail!("preview mutated file");
    }
    let applied = child.request(json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"tfy_output_apply","arguments":{"session":"smoke","restore_payload":restore_payload}}}))?;
    let applied_json = mcp_content_json(&applied)?;
    if applied_json["applied"] != true {
        bail!("apply did not report applied");
    }
    let after = fs::read_to_string(&sample)?;
    if !(after.contains("return total*2;") || after.contains("return total * 2;")) {
        bail!("apply result missing expected edit: {after}");
    }
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
            "tfy_scope_list returned an exact scope id".into(),
            "tfy_context_get returned compact code, symbol map, context_ref, and ApplyProof".into(),
            "tfy_output_validate did not mutate the file".into(),
            "tfy_output_apply mutated only through proof-gated apply".into(),
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
