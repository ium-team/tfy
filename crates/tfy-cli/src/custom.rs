use anyhow::{bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tfy_core::{summarize_command_output_bytes_with_rules, CommandRuleSet};

use crate::product::{prompt_menu_tui, raw_terminal_unavailable};
use crate::util::print_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CustomScope {
    Repo,
    Global,
}

impl CustomScope {
    fn source_kind(self) -> &'static str {
        match self {
            CustomScope::Repo => "repo",
            CustomScope::Global => "global_custom",
        }
    }

    fn cli_flag(self) -> &'static str {
        match self {
            CustomScope::Repo => "repo",
            CustomScope::Global => "global",
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum CustomCmd {
    /// Initialize the selected TFY custom-rule harness workspace.
    Init {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        json: bool,
    },
    /// Capture one command's output as a scoped rule fixture without trusting rules.
    Capture {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        name: String,
        #[arg(long)]
        json: bool,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Import an existing output sample as a scoped rule fixture.
    ImportFixture {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        name: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print/write bounded instructions for the user's chosen AI agent.
    Prompt {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long, default_value = "generic")]
        agent: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Validate, preview, compare, and record evidence for one fixture/rule pair.
    Verify {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        name: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        allow_legacy_global_rules: bool,
        #[arg(long)]
        accept_larger_than_built_in: bool,
    },
    /// Trust verified scoped custom rules by writing schema v2 provenance.
    Trust {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        name: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Report repo-local, global custom, and legacy user-global trust state separately.
    Status {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Convenience flow: init, capture, prompt, and optionally verify+trust.
    Create {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, value_enum, default_value_t = CustomScope::Repo)]
        scope: CustomScope,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "generic")]
        agent: String,
        #[arg(long)]
        verify_and_trust: bool,
        #[arg(long)]
        allow_legacy_global_rules: bool,
        #[arg(long)]
        accept_larger_than_built_in: bool,
        #[arg(long)]
        json: bool,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}

#[derive(Serialize)]
struct CustomResponse<T: Serialize> {
    ok: bool,
    command: &'static str,
    #[serde(flatten)]
    payload: T,
}

#[derive(Serialize)]
struct InitPayload {
    repo: String,
    allowed_paths: Vec<String>,
    forbidden_paths: Vec<String>,
    created_paths: Vec<String>,
    rule_file: String,
    fixture_dir: String,
    custom_dir: String,
    instruction_template: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct FixtureMetadata {
    schema_version: u8,
    name: String,
    cmd: Vec<String>,
    command_display: String,
    fixture_path: String,
    fixture_sha256: String,
    exit_code: i32,
    captured_at: String,
    source: String,
}

#[derive(Serialize)]
struct PromptPayload {
    agent: String,
    name: String,
    instruction_file: String,
    allowed_paths: Vec<String>,
    forbidden_actions: Vec<String>,
    verify_command: String,
    prompt: String,
}

#[derive(Serialize, Deserialize)]
struct VerifyEvidence {
    schema_version: u8,
    name: String,
    verified: bool,
    rules_sha256: String,
    fixture_sha256: String,
    validated_at: String,
    validated_with: Vec<String>,
    allow_legacy_global_rules: bool,
    user_global_rules_legacy_manual: bool,
    accept_larger_than_built_in: bool,
    command_display: String,
    cmd: Vec<String>,
    preview_strategy_kind: String,
    preview_rule_id: Option<String>,
    preview_raw_ref_present: bool,
    comparison: serde_json::Value,
    override_evidence: Vec<OverrideEvidence>,
}

#[derive(Serialize, Deserialize)]
struct OverrideEvidence {
    rule_id: String,
    family: String,
    override_active: bool,
}

pub(crate) fn execute_custom(cmd: Option<CustomCmd>) -> Result<()> {
    let Some(cmd) = cmd else {
        return execute_custom_wizard();
    };
    match cmd {
        CustomCmd::Init { repo, scope, json } => execute_init(repo, scope, json),
        CustomCmd::Capture {
            repo,
            scope,
            name,
            json,
            command,
        } => execute_capture(repo, scope, name, command, json),
        CustomCmd::ImportFixture {
            repo,
            scope,
            name,
            file,
            json,
        } => execute_import_fixture(repo, scope, name, file, json),
        CustomCmd::Prompt {
            repo,
            scope,
            agent,
            name,
            json,
        } => execute_prompt(repo, scope, agent, name, json),
        CustomCmd::Verify {
            repo,
            scope,
            name,
            json,
            allow_legacy_global_rules,
            accept_larger_than_built_in,
        } => execute_verify(
            repo,
            scope,
            name,
            allow_legacy_global_rules,
            accept_larger_than_built_in,
            json,
        ),
        CustomCmd::Trust {
            repo,
            scope,
            name,
            dry_run,
            json,
        } => execute_trust(repo, scope, name, dry_run, json),
        CustomCmd::Status { repo, json } => execute_status(repo, json),
        CustomCmd::Create {
            repo,
            scope,
            name,
            agent,
            verify_and_trust,
            allow_legacy_global_rules,
            accept_larger_than_built_in,
            json,
            command,
        } => execute_create(CreateRequest {
            repo,
            scope,
            name,
            agent,
            command,
            verify_and_trust,
            allow_legacy_global_rules,
            accept_larger_than_built_in,
            json,
        }),
    }
}

fn execute_custom_wizard() -> Result<()> {
    match prompt_custom_scope()? {
        CustomScopeChoice::Repo => {
            let layout = prepare_workspace(PathBuf::from("."), CustomScope::Repo)?;
            println!("tfy custom: repo scope selected");
            println!("repo={}", layout.repo.display());
            println!("rules={}", layout.display_path(&layout.commands));
            println!("fixtures={}", layout.display_path(&layout.fixtures_dir));
            println!(
                "next: run `tfy custom capture --name <name> -- <command>` or `tfy custom import-fixture --name <name> --file <output.txt>`"
            );
            println!(
                "then: ask your agent using `.tfy/custom/AGENT_INSTRUCTIONS.md`, run `tfy custom verify --name <name>`, then `tfy custom trust --name <name>`"
            );
            Ok(())
        }
        CustomScopeChoice::Global => {
            let layout = prepare_workspace(PathBuf::from("."), CustomScope::Global)?;
            println!("tfy custom: global scope selected");
            println!("execution_cwd={}", layout.repo.display());
            println!("rules={}", layout.display_path(&layout.commands));
            println!("fixtures={}", layout.display_path(&layout.fixtures_dir));
            println!(
                "next: run `tfy custom capture --scope global --name <name> -- <command>` or `tfy custom import-fixture --scope global --name <name> --file <output.txt>`"
            );
            println!(
                "then: ask your agent using `{}/AGENT_INSTRUCTIONS.md`, run `tfy custom verify --scope global --name <name>`, then `tfy custom trust --scope global --name <name>`",
                layout.custom_dir.display()
            );
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CustomScopeChoice {
    Repo,
    Global,
}

fn prompt_custom_scope() -> Result<CustomScopeChoice> {
    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        let choices = [
            "Current repo (.tfy/) — recommended",
            "Global (~/.config/tfy/custom/) — cross-repo",
            "Cancel",
        ];
        match prompt_menu_tui("TFY custom: choose rule scope", &choices) {
            Ok(0) => return Ok(CustomScopeChoice::Repo),
            Ok(1) => return Ok(CustomScopeChoice::Global),
            Ok(_) => bail!("tfy custom cancelled"),
            Err(err) if raw_terminal_unavailable(&err) => {
                eprintln!("TFY custom: TUI unavailable ({err}); falling back to line prompt");
            }
            Err(err) => return Err(err),
        }
    }
    eprintln!("TFY custom: choose rule scope: repo or global");
    let input = read_custom_prompt_input()?;
    let choice = input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    parse_custom_scope_choice(choice)
}

fn read_custom_prompt_input() -> Result<String> {
    let stdin = std::io::stdin();
    let mut input = String::new();
    if stdin.is_terminal() {
        stdin.lock().read_line(&mut input)?;
    } else {
        stdin.lock().read_to_string(&mut input)?;
    }
    Ok(input)
}

fn parse_custom_scope_choice(choice: &str) -> Result<CustomScopeChoice> {
    match choice.trim().to_ascii_lowercase().as_str() {
        "1" | "r" | "repo" | "current" | "local" => Ok(CustomScopeChoice::Repo),
        "2" | "g" | "global" => Ok(CustomScopeChoice::Global),
        "3" | "c" | "cancel" | "q" | "quit" => bail!("tfy custom cancelled"),
        _ => bail!("tfy custom requires an explicit scope choice: repo or global"),
    }
}

fn execute_init(repo: PathBuf, scope: CustomScope, json: bool) -> Result<()> {
    let layout = prepare_workspace(repo, scope)?;
    let payload = init_payload(&layout, "generic")?;
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: "init",
            payload,
        })
    } else {
        println!(
            "tfy custom init: ok scope={} execution_cwd={}",
            layout.scope_label(),
            layout.repo.display()
        );
        println!("allowed_paths={}", allowed_paths(&layout).join(" "));
        Ok(())
    }
}

fn execute_capture(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    mut command: Vec<String>,
    json: bool,
) -> Result<()> {
    let layout = prepare_workspace(repo, scope)?;
    let name = safe_name(&name)?;
    if command.first().map(|arg| arg == "--").unwrap_or(false) {
        command.remove(0);
    }
    let meta = capture_command_fixture(&layout, &name, command)?;
    emit_fixture_metadata(&layout, "capture", meta, json)
}

fn execute_import_fixture(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    file: PathBuf,
    json: bool,
) -> Result<()> {
    let layout = prepare_workspace(repo, scope)?;
    let name = safe_name(&name)?;
    let file = canonicalize_existing_file(&file)?;
    let bytes =
        fs::read(&file).with_context(|| format!("read fixture source {}", file.display()))?;
    let meta = write_fixture_and_metadata(
        &layout,
        &name,
        vec![name.clone()],
        bytes,
        0,
        "import-fixture",
    )?;
    emit_fixture_metadata(&layout, "import-fixture", meta, json)
}

fn execute_prompt(
    repo: PathBuf,
    scope: CustomScope,
    agent: String,
    name: String,
    json: bool,
) -> Result<()> {
    let layout = prepare_workspace(repo, scope)?;
    let name = safe_name(&name)?;
    let meta = read_metadata(&layout, &name)?;
    let payload = write_prompt_payload(&layout, agent, &name, &meta)?;
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: "prompt",
            payload,
        })
    } else {
        println!(
            "tfy custom prompt: ok instruction_file={}",
            payload.instruction_file
        );
        println!("{}", payload.prompt);
        Ok(())
    }
}

fn verify_custom(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    allow_legacy_global_rules: bool,
    accept_larger_than_built_in: bool,
) -> Result<VerifyEvidence> {
    let layout = prepare_workspace(repo, scope)?;
    let name = safe_name(&name)?;
    let global_rules = user_global_rules_path();
    let has_global = global_rules.as_ref().is_some_and(|path| path.exists());
    if has_global && !allow_legacy_global_rules {
        bail!("user-global command rules exist at {}; rerun with --allow-legacy-global-rules to explicitly record this legacy/manual influence", global_rules.unwrap().display());
    }
    reject_symlink(&layout.tfy_dir)?;
    reject_symlink(&layout.commands)?;
    let meta = read_metadata(&layout, &name)?;
    let fixture = layout.fixture_path_from_meta(&meta);
    ensure_inside(&layout.fixtures_dir, &fixture)?;
    reject_symlink(&fixture)?;
    let fixture_bytes = fs::read(&fixture)?;
    let actual_fixture_hash = sha256_hex(&fixture_bytes);
    if actual_fixture_hash != meta.fixture_sha256 {
        bail!(
            "fixture hash mismatch for {name}; expected {}, got {actual_fixture_hash}",
            meta.fixture_sha256
        );
    }
    let rules = CommandRuleSet::load_strict(&layout.commands, layout.scope.source_kind())?;
    let rules_sha256 = sha256_hex(&fs::read(&layout.commands)?);
    let argv = if meta.cmd.is_empty() {
        vec![name.clone()]
    } else {
        meta.cmd.clone()
    };
    let raw_root = prepare_custom_raw_dir(&layout, &name)?;
    let summary = summarize_command_output_bytes_with_rules(
        &meta.command_display,
        &argv,
        &fixture_bytes,
        meta.exit_code,
        raw_root.clone(),
        Some(&rules),
    )?;
    if summary.strategy_kind != "user_toml" {
        bail!(
            "custom verify fixture {name} did not exercise a user TOML rule; got strategy_kind={}",
            summary.strategy_kind
        );
    }
    if summary.rule_id.is_none() {
        bail!("custom verify fixture {name} did not produce a rule_id");
    }
    if summary.rendering_kind == "summary" && summary.raw_ref.is_empty() {
        bail!("custom preview summarized without a recoverable raw_ref");
    }
    if contains_unredacted_secret_like(&summary.model_text) {
        bail!("custom preview appears to leak a secret-like value in model-visible output");
    }
    let without_rules = summarize_command_output_bytes_with_rules(
        &meta.command_display,
        &argv,
        &fixture_bytes,
        meta.exit_code,
        raw_root.join("without-rules"),
        None,
    )?;
    let override_active = summary
        .command_rule_diagnostics
        .iter()
        .any(|d| d.code == "user_rule_overrode_builtin");
    let custom_larger = summary.summary_chars > without_rules.summary_chars;
    if override_active && custom_larger && !accept_larger_than_built_in {
        bail!("custom override output is larger than built-in/default; rerun verify with --accept-larger-than-built-in to record explicit acceptance");
    }
    let comparison = serde_json::json!({
        "override_active": override_active,
        "with_rules_strategy_kind": summary.strategy_kind,
        "without_rules_strategy_kind": without_rules.strategy_kind,
        "with_rules_summary_chars": summary.summary_chars,
        "without_rules_summary_chars": without_rules.summary_chars,
        "with_rules_smaller_than_without_rules": summary.summary_chars < without_rules.summary_chars,
        "with_rules_saved_chars_vs_without_rules": without_rules.summary_chars.saturating_sub(summary.summary_chars),
        "with_rules_larger_chars_vs_without_rules": summary.summary_chars.saturating_sub(without_rules.summary_chars),
    });
    let override_evidence = if override_active {
        let rule_id = summary
            .rule_id
            .clone()
            .context("override compare evidence is missing rule_id")?;
        vec![OverrideEvidence {
            rule_id,
            family: summary.command_family.clone(),
            override_active: true,
        }]
    } else {
        Vec::new()
    };
    let evidence = VerifyEvidence {
        schema_version: 1,
        name: name.clone(),
        verified: true,
        rules_sha256,
        fixture_sha256: actual_fixture_hash,
        validated_at: now_stamp(),
        validated_with: vec![
            "validate".into(),
            "preview".into(),
            "compare-built-in".into(),
        ],
        allow_legacy_global_rules,
        user_global_rules_legacy_manual: has_global,
        accept_larger_than_built_in,
        command_display: meta.command_display,
        cmd: argv,
        preview_strategy_kind: summary.strategy_kind,
        preview_rule_id: summary.rule_id,
        preview_raw_ref_present: !summary.raw_ref.is_empty(),
        comparison,
        override_evidence,
    };
    let evidence_path = layout.custom_dir.join(format!("{name}.verify.json"));
    reject_existing_symlink(&evidence_path)?;
    fs::write(&evidence_path, serde_json::to_string_pretty(&evidence)?)?;
    Ok(evidence)
}

fn execute_verify(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    allow_legacy_global_rules: bool,
    accept_larger_than_built_in: bool,
    json: bool,
) -> Result<()> {
    let evidence = verify_custom(
        repo.clone(),
        scope,
        name.clone(),
        allow_legacy_global_rules,
        accept_larger_than_built_in,
    )?;
    let layout = workspace_paths(repo, scope)?;
    let evidence_path = layout.custom_dir.join(format!("{name}.verify.json"));
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: "verify",
            payload: &evidence,
        })
    } else {
        println!(
            "tfy custom verify: ok name={name} evidence={}",
            layout.display_path(&evidence_path)
        );
        Ok(())
    }
}

fn trust_custom(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    dry_run: bool,
) -> Result<(serde_json::Value, PathBuf, PathBuf)> {
    let layout = prepare_workspace(repo, scope)?;
    let name = safe_name(&name)?;
    reject_symlink(&layout.tfy_dir)?;
    reject_symlink(&layout.commands)?;
    let meta = read_metadata(&layout, &name)?;
    let evidence_path = layout.custom_dir.join(format!("{name}.verify.json"));
    reject_symlink(&evidence_path)?;
    let evidence: VerifyEvidence = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    if !evidence.verified {
        bail!("verify evidence for {name} is not marked verified");
    }
    let current_rules_hash = sha256_hex(&fs::read(&layout.commands)?);
    if current_rules_hash != evidence.rules_sha256 {
        bail!("rule bytes changed after verify; rerun tfy custom verify");
    }
    let fixture = layout.fixture_path_from_meta(&meta);
    ensure_inside(&layout.fixtures_dir, &fixture)?;
    reject_symlink(&fixture)?;
    let current_fixture_hash = sha256_hex(&fs::read(&fixture)?);
    if current_fixture_hash != evidence.fixture_sha256
        || current_fixture_hash != meta.fixture_sha256
    {
        bail!("fixture bytes changed after verify; rerun tfy custom verify");
    }
    if evidence.comparison["override_active"].as_bool() == Some(true)
        && evidence.override_evidence.is_empty()
    {
        bail!("override compare evidence is missing");
    }
    let trust_file = layout.trust_file();
    reject_existing_symlink(&trust_file)?;
    let trust = serde_json::json!({
        "schema_version": 2,
        "command_rules": {
            "trusted": true,
            "rules_sha256": evidence.rules_sha256,
            "created_by": "tfy custom",
            "validated_at": evidence.validated_at,
            "validated_with": evidence.validated_with,
            "fixtures": [{
                "name": name,
                "fixture_sha256": evidence.fixture_sha256,
                "cmd": evidence.cmd,
                "path": meta.fixture_path,
            }],
            "agent": {"kind": "user", "bounded_workspace": true},
            "allow_legacy_global_rules": evidence.allow_legacy_global_rules,
            "user_global_rules_legacy_manual": evidence.user_global_rules_legacy_manual,
            "accept_larger_than_built_in": evidence.accept_larger_than_built_in,
            "override_evidence": evidence.override_evidence,
        }
    });
    if !dry_run {
        fs::write(&trust_file, serde_json::to_string_pretty(&trust)?)?;
    }
    Ok((trust, layout.repo, trust_file))
}

fn execute_trust(
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let (trust, repo, trust_file) = trust_custom(repo, scope, name, dry_run)?;
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: "trust",
            payload: trust,
        })
    } else {
        println!(
            "tfy custom trust: ok trust_file={} dry_run={dry_run}",
            display_repo_path(&repo, &trust_file)
        );
        Ok(())
    }
}

fn execute_status(repo: PathBuf, json: bool) -> Result<()> {
    let repo_layout = workspace_paths(repo.clone(), CustomScope::Repo)?;
    let global_layout = workspace_paths(repo, CustomScope::Global)?;
    let repo_status = custom_layout_status(&repo_layout)?;
    let global_status = custom_layout_status(&global_layout)?;
    let legacy_path = user_global_rules_path();
    let legacy_exists = legacy_path.as_ref().is_some_and(|p| p.exists());
    let payload = serde_json::json!({
        "repo": repo_layout.repo.display().to_string(),
        "repo_local": repo_status,
        "global_custom": global_status,
        "user_global_legacy": {
            "path": legacy_path.map(|p| p.display().to_string()),
            "exists": legacy_exists,
            "state": if legacy_exists { "present_legacy_manual" } else { "missing" },
            "diagnostic": if legacy_exists { "user_global_rules_legacy_manual" } else { "" },
        }
    });
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: "status",
            payload,
        })
    } else {
        println!(
            "tfy custom status: repo_local={} global_custom={} user_global_legacy={}",
            payload["repo_local"]["state"].as_str().unwrap_or("unknown"),
            payload["global_custom"]["state"]
                .as_str()
                .unwrap_or("unknown"),
            payload["user_global_legacy"]["state"]
                .as_str()
                .unwrap_or("unknown")
        );
        Ok(())
    }
}

fn runtime_diagnostic_state(layout: &CustomLayout) -> Option<&'static str> {
    let diagnostics = CommandRuleSet::load_standard(&layout.repo)
        .diagnostics()
        .to_vec();
    match layout.scope {
        CustomScope::Repo => diagnostics
            .iter()
            .find(|d| d.source_kind == "repo")
            .and_then(|d| match d.code.as_str() {
                "repo_rules_untrusted" => Some("untrusted"),
                "repo_rules_hash_mismatch" if d.message.contains("changed after trust") => {
                    Some("stale")
                }
                "repo_rules_hash_mismatch" => Some("invalid"),
                _ => None,
            }),
        CustomScope::Global => diagnostics
            .iter()
            .find(|d| d.source_kind == "global_custom")
            .and_then(|d| match d.code.as_str() {
                "global_custom_rules_untrusted" => Some("untrusted"),
                "global_custom_rules_hash_mismatch" => Some("stale"),
                "global_custom_rules_invalid_trust" | "global_custom_rules_symlink_refused" => {
                    Some("invalid")
                }
                _ => None,
            }),
    }
}

fn custom_layout_status(layout: &CustomLayout) -> Result<serde_json::Value> {
    let rules_exists = layout.commands.exists();
    let trust_file = layout.trust_file();
    let mut trust_schema_version = 0_u64;
    let state = if !rules_exists {
        "missing"
    } else if !trust_file.exists() {
        "untrusted"
    } else if reject_existing_symlink(&layout.tfy_dir).is_err()
        || reject_existing_symlink(&layout.commands).is_err()
        || reject_existing_symlink(&trust_file).is_err()
    {
        "invalid"
    } else {
        match fs::read(&trust_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        {
            Some(trust) => {
                trust_schema_version = trust["schema_version"].as_u64().unwrap_or(0);
                let expected = trust["command_rules"]["rules_sha256"]
                    .as_str()
                    .unwrap_or("");
                let actual = sha256_hex(&fs::read(&layout.commands)?);
                if expected != actual || expected.is_empty() {
                    "stale"
                } else if let Some(state) = runtime_diagnostic_state(layout) {
                    state
                } else {
                    match (layout.scope, trust_schema_version) {
                        (CustomScope::Repo, 1) => "trusted_v1_legacy",
                        (_, 2) => "trusted_v2",
                        (CustomScope::Global, 1) => "invalid",
                        _ => "invalid",
                    }
                }
            }
            None => "invalid",
        }
    };
    Ok(serde_json::json!({
        "rules_file": layout.display_path(&layout.commands),
        "rules_exists": rules_exists,
        "trust_file": layout.display_path(&trust_file),
        "state": state,
        "trust_schema_version": trust_schema_version,
    }))
}

struct CreateRequest {
    repo: PathBuf,
    scope: CustomScope,
    name: String,
    agent: String,
    command: Vec<String>,
    verify_and_trust: bool,
    allow_legacy_global_rules: bool,
    accept_larger_than_built_in: bool,
    json: bool,
}

fn execute_create(request: CreateRequest) -> Result<()> {
    let layout = prepare_workspace(request.repo.clone(), request.scope)?;
    let name = safe_name(&request.name)?;
    let meta = capture_command_fixture(&layout, &name, request.command)?;
    let prompt = write_prompt_payload(&layout, request.agent, &name, &meta)?;
    if request.verify_and_trust {
        let evidence = verify_custom(
            request.repo.clone(),
            request.scope,
            request.name.clone(),
            request.allow_legacy_global_rules,
            request.accept_larger_than_built_in,
        )?;
        let (trust, _, _) = trust_custom(request.repo, request.scope, request.name.clone(), false)?;
        if request.json {
            print_json(&serde_json::json!({
                "ok": true,
                "command": "create",
                "stopped_before_trust": false,
                "fixture": meta,
                "prompt": prompt,
                "verify": evidence,
                "trust": trust,
            }))
        } else {
            println!("tfy custom capture: ok fixture={}", meta.fixture_path);
            println!(
                "tfy custom prompt: ok instruction_file={}",
                prompt.instruction_file
            );
            println!("tfy custom verify: ok name={}", request.name);
            println!("tfy custom trust: ok");
            Ok(())
        }
    } else if request.json {
        print_json(&serde_json::json!({
            "ok": true,
            "command": "create",
            "stopped_before_trust": true,
            "fixture": meta,
            "prompt": prompt,
        }))
    } else {
        println!("tfy custom capture: ok fixture={}", meta.fixture_path);
        println!(
            "tfy custom prompt: ok instruction_file={}",
            prompt.instruction_file
        );
        println!(
            "tfy custom create: stopped before trust; edit {} then run tfy custom verify/trust",
            layout.display_path(&layout.commands)
        );
        Ok(())
    }
}

struct CustomLayout {
    scope: CustomScope,
    repo: PathBuf,
    tfy_dir: PathBuf,
    commands: PathBuf,
    fixtures_dir: PathBuf,
    custom_dir: PathBuf,
    created_paths: Vec<String>,
}

impl CustomLayout {
    fn scope_label(&self) -> &'static str {
        self.scope.cli_flag()
    }

    fn display_path(&self, path: &Path) -> String {
        match self.scope {
            CustomScope::Repo => display_repo_path(&self.repo, path),
            CustomScope::Global => path.display().to_string(),
        }
    }

    fn trust_file(&self) -> PathBuf {
        self.tfy_dir.join("trust.json")
    }

    fn fixture_path_from_meta(&self, meta: &FixtureMetadata) -> PathBuf {
        let path = PathBuf::from(&meta.fixture_path);
        if path.is_absolute() {
            path
        } else {
            self.repo.join(path)
        }
    }
}

fn workspace_paths(repo: PathBuf, scope: CustomScope) -> Result<CustomLayout> {
    let repo = canonicalize_existing_dir(&repo)?;
    let tfy_dir = match scope {
        CustomScope::Repo => repo.join(".tfy"),
        CustomScope::Global => global_custom_root()?,
    };
    let commands = tfy_dir.join("commands.toml");
    let fixtures_dir = tfy_dir.join("rule-fixtures");
    let custom_dir = match scope {
        CustomScope::Repo => tfy_dir.join("custom"),
        CustomScope::Global => tfy_dir.join("agent"),
    };
    Ok(CustomLayout {
        scope,
        repo,
        tfy_dir,
        commands,
        fixtures_dir,
        custom_dir,
        created_paths: Vec::new(),
    })
}

fn prepare_workspace(repo: PathBuf, scope: CustomScope) -> Result<CustomLayout> {
    let mut layout = workspace_paths(repo, scope)?;
    reject_existing_symlink(&layout.tfy_dir)?;
    if !layout.tfy_dir.exists() {
        fs::create_dir_all(&layout.tfy_dir)?;
        layout
            .created_paths
            .push(layout.display_path(&layout.tfy_dir));
    }
    reject_symlink(&layout.tfy_dir)?;
    reject_existing_symlink(&layout.commands)?;
    reject_existing_symlink(&layout.fixtures_dir)?;
    reject_existing_symlink(&layout.custom_dir)?;
    if !layout.fixtures_dir.exists() {
        fs::create_dir_all(&layout.fixtures_dir)?;
        layout
            .created_paths
            .push(layout.display_path(&layout.fixtures_dir));
    }
    if !layout.custom_dir.exists() {
        fs::create_dir_all(&layout.custom_dir)?;
        layout
            .created_paths
            .push(layout.display_path(&layout.custom_dir));
    }
    if !layout.commands.exists() {
        fs::write(&layout.commands, "# TFY custom command rules. Edit through `tfy custom` harness and verify before trust.\nschema_version = 3\n")?;
        layout
            .created_paths
            .push(layout.display_path(&layout.commands));
    }
    let template = layout.custom_dir.join("AGENT_INSTRUCTIONS.md");
    reject_existing_symlink(&template)?;
    if !template.exists() {
        fs::write(&template, base_agent_instructions(&layout, "generic"))?;
        layout.created_paths.push(layout.display_path(&template));
    }
    Ok(layout)
}

fn init_payload(layout: &CustomLayout, agent: &str) -> Result<InitPayload> {
    Ok(InitPayload {
        repo: layout.repo.display().to_string(),
        allowed_paths: allowed_paths(layout),
        forbidden_paths: forbidden_paths(layout),
        created_paths: layout.created_paths.clone(),
        rule_file: layout.display_path(&layout.commands),
        fixture_dir: layout.display_path(&layout.fixtures_dir),
        custom_dir: layout.display_path(&layout.custom_dir),
        instruction_template: base_agent_instructions(layout, agent),
    })
}

fn capture_command_fixture(
    layout: &CustomLayout,
    name: &str,
    mut command: Vec<String>,
) -> Result<FixtureMetadata> {
    if command.first().map(|arg| arg == "--").unwrap_or(false) {
        command.remove(0);
    }
    validate_command_args_no_secrets(&command)?;
    let Some((program, args)) = command.split_first() else {
        bail!("tfy custom capture requires a command after --");
    };
    let output = Command::new(program)
        .current_dir(&layout.repo)
        .args(args)
        .output()
        .with_context(|| format!("run capture command {program}"))?;
    let mut bytes = Vec::with_capacity(output.stdout.len() + output.stderr.len());
    bytes.extend_from_slice(&output.stdout);
    bytes.extend_from_slice(&output.stderr);
    write_fixture_and_metadata(
        layout,
        name,
        command,
        bytes,
        output.status.code().unwrap_or(1),
        "capture",
    )
}

fn write_fixture_and_metadata(
    layout: &CustomLayout,
    name: &str,
    command: Vec<String>,
    bytes: Vec<u8>,
    exit_code: i32,
    source: &str,
) -> Result<FixtureMetadata> {
    let fixture = layout.fixtures_dir.join(format!("{name}.txt"));
    let meta_path = layout.custom_dir.join(format!("{name}.json"));
    reject_existing_symlink(&fixture)?;
    reject_existing_symlink(&meta_path)?;
    fs::write(&fixture, &bytes)?;
    let meta = FixtureMetadata {
        schema_version: 1,
        name: name.into(),
        command_display: redact_command_display(&command),
        cmd: command,
        fixture_path: layout.display_path(&fixture),
        fixture_sha256: sha256_hex(&bytes),
        exit_code,
        captured_at: now_stamp(),
        source: source.into(),
    };
    fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    Ok(meta)
}

fn emit_fixture_metadata(
    _layout: &CustomLayout,
    command_name: &'static str,
    meta: FixtureMetadata,
    json: bool,
) -> Result<()> {
    if json {
        print_json(&CustomResponse {
            ok: true,
            command: command_name,
            payload: meta,
        })
    } else {
        println!(
            "tfy custom {command_name}: ok fixture={}",
            meta.fixture_path
        );
        Ok(())
    }
}

fn read_metadata(layout: &CustomLayout, name: &str) -> Result<FixtureMetadata> {
    let path = layout.custom_dir.join(format!("{name}.json"));
    reject_symlink(&path)?;
    serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("read custom metadata {}", path.display()))?,
    )
    .map_err(Into::into)
}

fn write_prompt_payload(
    layout: &CustomLayout,
    agent: String,
    name: &str,
    meta: &FixtureMetadata,
) -> Result<PromptPayload> {
    let instruction_file = layout.custom_dir.join(format!("{name}.instructions.md"));
    let prompt = agent_prompt(layout, &agent, name, meta);
    reject_existing_symlink(&instruction_file)?;
    fs::write(&instruction_file, &prompt)?;
    Ok(PromptPayload {
        agent,
        name: name.to_string(),
        instruction_file: layout.display_path(&instruction_file),
        allowed_paths: allowed_paths(layout),
        forbidden_actions: forbidden_actions(),
        verify_command: format!(
            "tfy custom verify --scope {} --repo . --name {name}",
            layout.scope.cli_flag()
        ),
        prompt,
    })
}

fn agent_prompt(layout: &CustomLayout, agent: &str, name: &str, meta: &FixtureMetadata) -> String {
    let edit_target = match layout.scope {
        CustomScope::Repo => {
            "`.tfy/commands.toml` and fixture files under `.tfy/rule-fixtures/`".to_string()
        }
        CustomScope::Global => format!(
            "`{}/commands.toml` and fixture files under `{}/rule-fixtures/`",
            layout.tfy_dir.display(),
            layout.tfy_dir.display()
        ),
    };
    format!("{}\n\nTask fixture: {name}\nCommand display: {}\nFixture: {}\n\nEdit only {edit_target}. After editing, run `tfy custom verify --scope {} --repo . --name {name}`. If you intentionally replace a built-in summary, use exact `[command.override]` family metadata and require compare evidence.\n",
        base_agent_instructions(layout, agent),
        meta.command_display,
        meta.fixture_path,
        layout.scope.cli_flag()
    )
}

fn base_agent_instructions(layout: &CustomLayout, agent: &str) -> String {
    match layout.scope {
        CustomScope::Repo => format!("# TFY custom rule authoring instructions for {agent}\n\nScope: repo-local `.tfy/` custom rules.\n\nAllowed writes:\n- `.tfy/commands.toml`\n- `.tfy/rule-fixtures/**`\n- `.tfy/custom/*.instructions.md` generated by TFY\n\nForbidden actions:\n- Do not edit Rust/source/config outside `.tfy/`.\n- Do not add scripts, hooks, subprocess launchers, plugin code, or official support claims.\n- Do not bypass raw-first, no-negative, redaction, trust, or plain-text-default invariants.\n- Do not claim TFY prevents OS-level edits; TFY invalidates trust/provenance when bytes change.\n"),
        CustomScope::Global => format!("# TFY custom rule authoring instructions for {agent}\n\nScope: global custom rules under `{}`. Commands may be captured from the current repo, but trust is anchored only in this global custom store.\n\nAllowed writes:\n- `{}/commands.toml`\n- `{}/rule-fixtures/**`\n- `{}/agent/*.instructions.md` generated by TFY\n\nForbidden actions:\n- Do not edit repo source, Rust/source/config, or files outside `{}`.\n- Do not edit legacy/manual `~/.config/tfy/commands.toml`.\n- Do not add scripts, hooks, subprocess launchers, plugin code, or official support claims.\n- Do not bypass raw-first, no-negative, redaction, trust, or plain-text-default invariants.\n- Do not claim TFY prevents OS-level edits; TFY invalidates trust/provenance when bytes change.\n",
            layout.tfy_dir.display(),
            layout.tfy_dir.display(),
            layout.tfy_dir.display(),
            layout.tfy_dir.display(),
            layout.tfy_dir.display()
        ),
    }
}

fn allowed_paths(layout: &CustomLayout) -> Vec<String> {
    match layout.scope {
        CustomScope::Repo => vec![
            ".tfy/commands.toml".into(),
            ".tfy/rule-fixtures/**".into(),
            ".tfy/custom/**".into(),
            "docs/COMMAND_RULES.md".into(),
            "docs/CUSTOM_COMMAND_RULE_AUTHORING.md".into(),
        ],
        CustomScope::Global => vec![
            format!("{}/commands.toml", layout.tfy_dir.display()),
            format!("{}/rule-fixtures/**", layout.tfy_dir.display()),
            format!("{}/agent/**", layout.tfy_dir.display()),
            "docs/COMMAND_RULES.md".into(),
            "docs/CUSTOM_COMMAND_RULE_AUTHORING.md".into(),
        ],
    }
}

fn forbidden_paths(layout: &CustomLayout) -> Vec<String> {
    match layout.scope {
        CustomScope::Repo => vec![
            "crates/**".into(),
            "docs/** except referenced docs".into(),
            ".github/**".into(),
            "scripts/**".into(),
        ],
        CustomScope::Global => vec![
            "repo source files".into(),
            "~/.config/tfy/commands.toml legacy/manual path".into(),
            "crates/**".into(),
            ".github/**".into(),
            "scripts/**".into(),
        ],
    }
}

fn forbidden_actions() -> Vec<String> {
    vec![
        "edit source outside selected TFY custom scope".into(),
        "add scripts/hooks/subprocess/plugin code".into(),
        "write trust without tfy custom verify".into(),
        "claim OS-level sandboxing".into(),
    ]
}

fn safe_name(name: &str) -> Result<String> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("unsafe custom fixture name: {name}; use ASCII letters, digits, '-' or '_'");
    }
    Ok(name.to_string())
}

fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize()?;
    if !path.is_dir() {
        bail!("not a directory: {}", path.display());
    }
    Ok(path)
}

fn canonicalize_existing_file(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize()?;
    if !path.is_file() {
        bail!("not a file: {}", path.display());
    }
    Ok(path)
}

fn reject_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!("tfy custom refuses symlink path: {}", path.display());
    }
    Ok(())
}

fn reject_existing_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("tfy custom refuses symlink path: {}", path.display())
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn ensure_inside(root: &Path, path: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root) {
        bail!("path escapes tfy custom harness: {}", path.display());
    }
    Ok(())
}

fn display_repo_path(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn prepare_custom_raw_dir(layout: &CustomLayout, name: &str) -> Result<PathBuf> {
    let custom_raw = layout.custom_dir.join("raw");
    reject_existing_symlink(&custom_raw)?;
    fs::create_dir_all(&custom_raw)?;
    reject_symlink(&custom_raw)?;
    ensure_inside(&layout.tfy_dir, &custom_raw)?;
    let raw_root = custom_raw.join(name);
    reject_existing_symlink(&raw_root)?;
    fs::create_dir_all(&raw_root)?;
    reject_symlink(&raw_root)?;
    ensure_inside(&layout.tfy_dir, &raw_root)?;
    Ok(raw_root)
}

fn validate_command_args_no_secrets(command: &[String]) -> Result<()> {
    if command.iter().any(|arg| contains_secret_like(arg)) {
        bail!("tfy custom capture refuses secret-like command arguments; use an input fixture file instead");
    }
    if command.iter().any(|arg| looks_like_opaque_secret_arg(arg)) {
        bail!("tfy custom capture refuses opaque high-entropy command arguments; use import-fixture instead");
    }
    Ok(())
}

fn redact_command_display(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| {
            if contains_secret_like(arg) {
                "[REDACTED]".to_string()
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_unredacted_secret_like(text: &str) -> bool {
    text.split_whitespace().any(contains_secret_like)
}

fn contains_secret_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("[redacted]") {
        return false;
    }
    let key_markers = [
        "token",
        "secret",
        "password",
        "passwd",
        "api_key",
        "apikey",
        "client_secret",
        "access_token",
        "session",
        "cookie",
        "authorization",
        "bearer",
    ];
    if key_markers.iter().any(|needle| lower.contains(needle)) {
        return true;
    }
    lower.starts_with("auth:") || lower.starts_with("authorization:")
}

fn looks_like_opaque_secret_arg(value: &str) -> bool {
    if value.len() < 24 || value.contains('/') || value.contains('\\') {
        return false;
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '=' | ':'))
    {
        return false;
    }
    let mut ascii_alnum = 0;
    let mut lowercase = false;
    let mut uppercase = false;
    let mut digit = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            ascii_alnum += 1;
            lowercase |= c.is_ascii_lowercase();
            uppercase |= c.is_ascii_uppercase();
            digit |= c.is_ascii_digit();
        }
    }
    ascii_alnum >= 20 && digit && (lowercase || uppercase)
}

fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before UNIX_EPOCH")
        .as_secs();
    format!("unix:{secs}")
}

fn global_custom_root() -> Result<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        bail!("HOME is required for tfy custom --scope global");
    };
    Ok(PathBuf::from(home)
        .join(".config")
        .join("tfy")
        .join("custom"))
}

fn user_global_rules_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("tfy")
            .join("commands.toml")
    })
}
