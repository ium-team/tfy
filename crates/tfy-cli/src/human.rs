use crate::gateways::execute_structured_tool_gateway_with_origin;
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tfy_runtime::{AdapterKind, Origin, RouteEvidence};

const START_MARKER: &str = "# TFY:HUMAN-SESSION:START";
const END_MARKER: &str = "# TFY:HUMAN-SESSION:END";
const SUPPORTED_SHELL: &str = "bash";
const ALLOWLIST: &[&str] = &[
    "git", "cargo", "npm", "node", "python", "python3", "pytest", "ls", "find", "grep", "rg", "gh",
    "sh",
];

#[derive(Subcommand)]
pub(crate) enum HumanCmd {
    /// Launch an explicit TFY-managed human shell session. Linux bash v1 only; no global terminal interception.
    Shell {
        #[arg(long, default_value = "human")]
        session: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value = ".tfy/human/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "bash")]
        shell: String,
    },
    /// Generate a sourceable Linux bash integration script for a TFY-managed human session.
    Install {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "human")]
        session: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value = ".tfy/human/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "bash")]
        shell: String,
    },
    /// Remove a TFY-owned generated human session script.
    Uninstall {
        #[arg(long)]
        path: PathBuf,
    },
    /// Internal command used by generated human-session shell functions.
    #[command(hide = true)]
    Run {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(
            long = "max-summary-bytes",
            alias = "max-output-bytes",
            default_value_t = 1_000_000
        )]
        max_summary_bytes: usize,
        #[arg(long, default_value = ".tfy/human/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "human")]
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

pub(crate) fn execute_human(cmd: HumanCmd) -> Result<()> {
    match cmd {
        HumanCmd::Shell {
            session,
            raw_dir,
            ledger,
            shell,
        } => execute_human_shell(&session, &raw_dir, &ledger, &shell),
        HumanCmd::Install {
            dry_run,
            output,
            session,
            raw_dir,
            ledger,
            shell,
        } => execute_human_install(dry_run, output, &session, &raw_dir, &ledger, &shell),
        HumanCmd::Uninstall { path } => execute_human_uninstall(&path),
        HumanCmd::Run {
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
            ensure_supported_shell(SUPPORTED_SHELL)?;
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
                AdapterKind::Shell,
                Origin::human_managed_session(),
                RouteEvidence::human_managed_session(),
            )
        }
    }
}

fn ensure_supported_shell(shell: &str) -> Result<()> {
    if shell != SUPPORTED_SHELL {
        bail!("tfy human shell supports Linux bash only in v1; unsupported shell: {shell}");
    }
    #[cfg(not(target_os = "linux"))]
    bail!("tfy human shell supports Linux bash only in v1; this platform is unsupported");
    #[cfg(target_os = "linux")]
    Ok(())
}

fn execute_human_shell(session: &str, raw_dir: &Path, ledger: &Path, shell: &str) -> Result<()> {
    ensure_supported_shell(shell)?;
    let integration_dir = PathBuf::from(".tfy/human");
    fs::create_dir_all(&integration_dir).context("create .tfy/human")?;
    let rcfile = integration_dir.join("session.bashrc");
    write_owned_script(&rcfile, &human_script(session, raw_dir, ledger, shell)?)?;
    let status = Command::new(shell)
        .arg("--rcfile")
        .arg(&rcfile)
        .arg("-i")
        .env("TFY_HUMAN_ACTIVE", "1")
        .env("TFY_HUMAN_SESSION", session)
        .env("TFY_HUMAN_RAW_DIR", raw_dir)
        .env("TFY_HUMAN_LEDGER", ledger)
        .status()
        .with_context(|| format!("launch {shell} for TFY human session"))?;
    std::process::exit(status.code().unwrap_or(1));
}

fn execute_human_install(
    dry_run: bool,
    output: Option<PathBuf>,
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    shell: &str,
) -> Result<()> {
    ensure_supported_shell(shell)?;
    let script = human_script(session, raw_dir, ledger, shell)?;
    if dry_run || output.is_none() {
        let path = output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".tfy/human/session.bashrc".into());
        println!("TFY human install dry-run");
        println!("shell=bash status=supported scope=explicit_tfy_managed_session_only");
        println!("would_write={path}");
        println!("source_command=source {}", shell_quote(&path));
        println!("script:\n{script}");
        return Ok(());
    }
    let output = output.expect("checked above");
    write_owned_script(&output, &script)?;
    println!("installed TFY human session script at {}", output.display());
    println!(
        "source it with: source {}",
        shell_quote(&output.display().to_string())
    );
    Ok(())
}

fn execute_human_uninstall(path: &Path) -> Result<()> {
    let existing = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if !is_owned_script(&existing) {
        bail!(
            "refusing to remove {}; missing TFY human session ownership markers",
            path.display()
        );
    }
    fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    println!("removed TFY human session script at {}", path.display());
    Ok(())
}

fn write_owned_script(path: &Path, script: &str) -> Result<()> {
    if path.exists() {
        let existing =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        if !is_owned_script(&existing) {
            bail!(
                "refusing to overwrite {}; missing TFY human session ownership markers",
                path.display()
            );
        }
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
    }
    fs::write(path, script).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn is_owned_script(text: &str) -> bool {
    text.contains(START_MARKER) && text.contains(END_MARKER)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn human_script(session: &str, raw_dir: &Path, ledger: &Path, shell: &str) -> Result<String> {
    ensure_supported_shell(shell)?;
    let session = shell_quote(session);
    let raw_dir = shell_quote(&raw_dir.display().to_string());
    let ledger = shell_quote(&ledger.display().to_string());
    let mut script = String::new();
    script.push_str(START_MARKER);
    script.push('\n');
    script.push_str("# TFY-managed human shell integration. Source only in shells where you want TFY command summaries.\n");
    script.push_str("# This is not global terminal interception; it only wraps the allowlisted functions below.\n");
    script.push_str(&format!("export TFY_HUMAN_ACTIVE=1\nexport TFY_HUMAN_SESSION={}\nexport TFY_HUMAN_RAW_DIR={}\nexport TFY_HUMAN_LEDGER={}\n", session, raw_dir, ledger));
    script.push_str("export PS1=\"(tfy-human) ${PS1:-$ }\"\n");
    script.push_str("_tfy_human_run() {\n  if [ \"${TFY_HUMAN_BYPASS:-}\" = \"1\" ]; then\n    command \"$@\"\n    return $?\n  fi\n  TFY_HUMAN_BYPASS=1 command tfy human run --session \"${TFY_HUMAN_SESSION:-human}\" --raw-dir \"${TFY_HUMAN_RAW_DIR:-.tfy/raw}\" --ledger \"${TFY_HUMAN_LEDGER:-.tfy/human/ledger.jsonl}\" -- \"$@\"\n}\n");
    for command in ALLOWLIST {
        script.push_str(&format!(
            "{0}() {{\n  _tfy_human_run {0} \"$@\"\n}}\n",
            command
        ));
    }
    script.push_str("tfy-human-bypass() {\n  TFY_HUMAN_BYPASS=1 command \"$@\"\n}\n");
    script.push_str(END_MARKER);
    script.push('\n');
    Ok(script)
}
