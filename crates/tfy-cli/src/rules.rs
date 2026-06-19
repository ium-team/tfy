use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tfy_core::{summarize_command_output_bytes_with_rules, CommandRuleSet};

use crate::util::print_json;

#[derive(Subcommand)]
pub(crate) enum RulesCmd {
    /// Strictly validate a command-rule TOML file without trusting or executing it.
    Validate {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "user")]
        source_kind: String,
        #[arg(long)]
        json: bool,
    },
    /// Preview one fixture through a rule file using TFY's raw-first/no-negative path.
    Preview {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        cmd: String,
        /// Explicit argv entries for commands with spaces/quotes; defaults to whitespace-splitting --cmd.
        #[arg(long = "arg")]
        argv: Vec<String>,
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long, default_value_t = 0)]
        exit_code: i32,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value = "user")]
        source_kind: String,
        #[arg(long)]
        json: bool,
    },
    /// Create the repo-local files/directories an agent may edit while authoring custom rules.
    AgentWorkspace {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Trust a reviewed repo-local .tfy/commands.toml by recording its sha256 in .tfy/trust.json.
    Trust {
        #[arg(long, default_value = ".tfy/commands.toml")]
        file: PathBuf,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Compare preview output with and without the supplied custom rules. Does not override built-ins.
    CompareBuiltIn {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        cmd: String,
        /// Explicit argv entries for commands with spaces/quotes; defaults to whitespace-splitting --cmd.
        #[arg(long = "arg")]
        argv: Vec<String>,
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long, default_value_t = 0)]
        exit_code: i32,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value = "user")]
        source_kind: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Serialize)]
struct RuleHarnessResponse<'a, T: Serialize> {
    ok: bool,
    command: &'a str,
    #[serde(flatten)]
    payload: T,
}

#[derive(Serialize)]
struct ValidatePayload {
    file: String,
    source_kind: String,
    diagnostics: Vec<tfy_core::CommandRuleDiagnostic>,
}

#[derive(Serialize)]
struct WorkspacePayload {
    repo: String,
    allowed_paths: Vec<String>,
    created_paths: Vec<String>,
    rule_file: String,
    trust_file: String,
}

#[derive(Serialize)]
struct TrustPayload {
    file: String,
    trust_file: String,
    rules_sha256: String,
    dry_run: bool,
}

struct RulePreviewRequest {
    file: PathBuf,
    cmd: String,
    argv: Vec<String>,
    fixture: PathBuf,
    exit_code: i32,
    raw_dir: PathBuf,
    source_kind: String,
    json: bool,
}

pub(crate) fn execute_rules(cmd: RulesCmd) -> Result<()> {
    match cmd {
        RulesCmd::Validate {
            file,
            source_kind,
            json,
        } => execute_validate(file, source_kind, json),
        RulesCmd::Preview {
            file,
            cmd,
            argv,
            fixture,
            exit_code,
            raw_dir,
            source_kind,
            json,
        } => execute_preview(RulePreviewRequest {
            file,
            cmd,
            argv,
            fixture,
            exit_code,
            raw_dir,
            source_kind,
            json,
        }),
        RulesCmd::AgentWorkspace { repo, json } => execute_agent_workspace(repo, json),
        RulesCmd::Trust {
            file,
            repo,
            dry_run,
            json,
        } => execute_trust(file, repo, dry_run, json),
        RulesCmd::CompareBuiltIn {
            file,
            cmd,
            argv,
            fixture,
            exit_code,
            raw_dir,
            source_kind,
            json,
        } => execute_compare(RulePreviewRequest {
            file,
            cmd,
            argv,
            fixture,
            exit_code,
            raw_dir,
            source_kind,
            json,
        }),
    }
}

fn execute_validate(file: PathBuf, source_kind: String, json: bool) -> Result<()> {
    let rules = CommandRuleSet::load_strict(&file, &source_kind)?;
    let payload = RuleHarnessResponse {
        ok: true,
        command: "validate",
        payload: ValidatePayload {
            file: file.display().to_string(),
            source_kind,
            diagnostics: rules.diagnostics().to_vec(),
        },
    };
    if json {
        print_json(&payload)
    } else {
        println!("tfy rules validate: ok file={}", file.display());
        Ok(())
    }
}

fn execute_preview(request: RulePreviewRequest) -> Result<()> {
    let rules = CommandRuleSet::load_strict(&request.file, &request.source_kind)?;
    let raw = fs::read(&request.fixture)
        .with_context(|| format!("read fixture {}", request.fixture.display()))?;
    let argv = preview_argv(&request.cmd, request.argv);
    let summary = summarize_command_output_bytes_with_rules(
        &request.cmd,
        &argv,
        &raw,
        request.exit_code,
        request.raw_dir,
        Some(&rules),
    )?;
    if request.json {
        print_json(&summary)
    } else {
        print!("{}", summary.model_text);
        Ok(())
    }
}

fn execute_compare(request: RulePreviewRequest) -> Result<()> {
    let rules = CommandRuleSet::load_strict(&request.file, &request.source_kind)?;
    let raw = fs::read(&request.fixture)
        .with_context(|| format!("read fixture {}", request.fixture.display()))?;
    let argv = preview_argv(&request.cmd, request.argv);
    let with_rules = summarize_command_output_bytes_with_rules(
        &request.cmd,
        &argv,
        &raw,
        request.exit_code,
        request.raw_dir.join("with-rules"),
        Some(&rules),
    )?;
    let without_rules = summarize_command_output_bytes_with_rules(
        &request.cmd,
        &argv,
        &raw,
        request.exit_code,
        request.raw_dir.join("without-rules"),
        None,
    )?;
    let value = serde_json::json!({
        "ok": true,
        "command": "compare-built-in",
        "note": "custom rules are compared additively; built-in override is not enabled",
        "with_rules": with_rules,
        "without_rules": without_rules,
    });
    if request.json {
        print_json(&value)
    } else {
        println!(
            "with_rules:\n{}",
            value["with_rules"]["model_text"].as_str().unwrap_or("")
        );
        println!(
            "\nwithout_rules:\n{}",
            value["without_rules"]["model_text"].as_str().unwrap_or("")
        );
        Ok(())
    }
}

fn execute_agent_workspace(repo: PathBuf, json: bool) -> Result<()> {
    let repo = canonicalize_existing_dir(&repo)?;
    let tfy = repo.join(".tfy");
    let fixtures = tfy.join("rule-fixtures");
    fs::create_dir_all(&fixtures)?;
    let mut created_paths = Vec::new();
    let commands = tfy.join("commands.toml");
    if !commands.exists() {
        fs::write(
            &commands,
            "# TFY custom command rules. Agents may edit this file and files under .tfy/rule-fixtures/.\n# Run `tfy rules validate --file .tfy/commands.toml` and `tfy rules preview ...` before trust.\nschema_version = 3\n",
        )?;
        created_paths.push(display_repo_path(&repo, &commands));
    }
    let gitkeep = fixtures.join(".gitkeep");
    if !gitkeep.exists() {
        fs::write(&gitkeep, "")?;
        created_paths.push(display_repo_path(&repo, &gitkeep));
    }
    let payload = RuleHarnessResponse {
        ok: true,
        command: "agent-workspace",
        payload: WorkspacePayload {
            repo: repo.display().to_string(),
            allowed_paths: vec![
                ".tfy/commands.toml".into(),
                ".tfy/rule-fixtures/**".into(),
                "docs/CUSTOM_COMMAND_RULE_AUTHORING.md".into(),
            ],
            created_paths,
            rule_file: display_repo_path(&repo, &commands),
            trust_file: display_repo_path(&repo, &tfy.join("trust.json")),
        },
    };
    if json {
        print_json(&payload)
    } else {
        println!("tfy rules agent-workspace: ok repo={}", repo.display());
        println!("allowed_paths=.tfy/commands.toml .tfy/rule-fixtures/** docs/CUSTOM_COMMAND_RULE_AUTHORING.md");
        Ok(())
    }
}

fn execute_trust(file: PathBuf, repo: PathBuf, dry_run: bool, json: bool) -> Result<()> {
    let repo = canonicalize_existing_dir(&repo)?;
    let file = if file.is_relative() {
        repo.join(file)
    } else {
        file
    };
    let file = canonicalize_existing_file(&file)?;
    let tfy_dir = repo.join(".tfy");
    reject_symlink(&tfy_dir)?;
    let commands_path = tfy_dir.join("commands.toml");
    reject_symlink(&commands_path)?;
    let expected = canonicalize_existing_file(&commands_path)?;
    if file != expected {
        bail!(
            "repo-scope trust only supports {}; got {}",
            expected.display(),
            file.display()
        );
    }
    CommandRuleSet::load_strict(&file, "repo")?;
    let bytes = fs::read(&file)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let trust_file = tfy_dir.join("trust.json");
    reject_existing_symlink(&trust_file)?;
    if !dry_run {
        fs::write(
            &trust_file,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "command_rules": {"trusted": true, "rules_sha256": hash}
            }))?,
        )?;
    }
    let payload = RuleHarnessResponse {
        ok: true,
        command: "trust",
        payload: TrustPayload {
            file: display_repo_path(&repo, &file),
            trust_file: display_repo_path(&repo, &trust_file),
            rules_sha256: hash,
            dry_run,
        },
    };
    if json {
        print_json(&payload)
    } else {
        println!(
            "tfy rules trust: ok file={} trust_file={} dry_run={}",
            display_repo_path(&repo, &file),
            display_repo_path(&repo, &trust_file),
            dry_run
        );
        Ok(())
    }
}

fn preview_argv(cmd: &str, argv: Vec<String>) -> Vec<String> {
    if argv.is_empty() {
        cmd.split_whitespace().map(str::to_string).collect()
    } else {
        argv
    }
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
        bail!("repo trust refuses symlink path: {}", path.display());
    }
    Ok(())
}

fn reject_existing_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("repo trust refuses symlink path: {}", path.display())
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn display_repo_path(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
