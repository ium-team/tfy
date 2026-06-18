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
        /// Disable PATH-shim auto-interception and keep only the managed shell environment.
        #[arg(long)]
        no_auto_intercept: bool,
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
            no_auto_intercept,
        } => execute_human_shell(&session, &raw_dir, &ledger, &shell, !no_auto_intercept),
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

pub(crate) fn execute_human_shell(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    shell: &str,
    auto_intercept: bool,
) -> Result<()> {
    ensure_supported_shell(shell)?;
    let integration_dir = PathBuf::from(".tfy/human");
    fs::create_dir_all(&integration_dir).context("create .tfy/human")?;
    let rcfile = integration_dir.join("session.bashrc");
    write_owned_script(
        &rcfile,
        &human_script(session, raw_dir, ledger, shell, auto_intercept)?,
    )?;
    let status = Command::new(shell)
        .arg("--rcfile")
        .arg(&rcfile)
        .arg("-i")
        .env("TFY_HUMAN_ACTIVE", "1")
        .env("TFY_HUMAN_SESSION", session)
        .env("TFY_HUMAN_RAW_DIR", raw_dir)
        .env("TFY_HUMAN_LEDGER", ledger)
        .env(
            "TFY_HUMAN_ROOT",
            std::env::current_dir()?
                .canonicalize()?
                .display()
                .to_string(),
        )
        .env(
            "TFY_HUMAN_AUTO_INTERCEPT",
            if auto_intercept { "1" } else { "0" },
        )
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
    let script = human_script(session, raw_dir, ledger, shell, true)?;
    if dry_run || output.is_none() {
        let path = output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".tfy/human/session.bashrc".into());
        println!("TFY human install dry-run");
        println!("shell=bash status=supported scope=project_scoped_tfy_managed_session");
        println!("would_write={path}");
        println!("source_command=source {}", shell_quote(&path));
        println!("script:\n{script}");
        return Ok(());
    }
    let output = output.expect("checked above");
    write_owned_script(&output, &script)?;
    println!("installed TFY human session script at {}", output.display());
    println!("shell=bash status=supported scope=project_scoped_tfy_managed_session");
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

fn human_script(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    _shell: &str,
    auto_intercept: bool,
) -> Result<String> {
    let session = shell_quote(session);
    let raw_dir = shell_quote(&raw_dir.display().to_string());
    let ledger = shell_quote(&ledger.display().to_string());
    let mut script = String::new();
    script.push_str(START_MARKER);
    script.push('\n');
    script.push_str("# TFY-managed human shell integration. Source only in shells where you want TFY command summaries.\n");
    script.push_str("# This is not global terminal interception; it routes PATH-resolved external commands only inside this managed bash session.\n");
    script.push_str(&format!(
        "export TFY_HUMAN_ACTIVE=1\nexport TFY_HUMAN_SESSION={}\n",
        session
    ));
    script.push_str("TFY_HUMAN_ROOT=\"$(cd -P -- \"${TFY_HUMAN_ROOT:-$PWD}\" 2>/dev/null && pwd -P)\" || TFY_HUMAN_ROOT=\"${TFY_HUMAN_ROOT:-$PWD}\"\nexport TFY_HUMAN_ROOT\n");
    script.push_str(&format!(
        "TFY_HUMAN_RAW_DIR_INPUT={}\ncase \"$TFY_HUMAN_RAW_DIR_INPUT\" in\n  /*) export TFY_HUMAN_RAW_DIR=\"$TFY_HUMAN_RAW_DIR_INPUT\" ;;\n  *) export TFY_HUMAN_RAW_DIR=\"${{TFY_HUMAN_ROOT%/}}/$TFY_HUMAN_RAW_DIR_INPUT\" ;;\nesac\n",
        raw_dir
    ));
    script.push_str(&format!(
        "TFY_HUMAN_LEDGER_INPUT={}\ncase \"$TFY_HUMAN_LEDGER_INPUT\" in\n  /*) export TFY_HUMAN_LEDGER=\"$TFY_HUMAN_LEDGER_INPUT\" ;;\n  *) export TFY_HUMAN_LEDGER=\"${{TFY_HUMAN_ROOT%/}}/$TFY_HUMAN_LEDGER_INPUT\" ;;\nesac\n",
        ledger
    ));
    script.push_str("export TFY_LAST_STATUS=0\n");
    script.push_str(if auto_intercept {
        "export TFY_HUMAN_AUTO_INTERCEPT=1\n"
    } else {
        "export TFY_HUMAN_AUTO_INTERCEPT=0\n"
    });
    script.push_str(
        "export PS1='(tfy-human:${TFY_HUMAN_ROOT}) [tfy exit=${TFY_LAST_STATUS:-0}] ${PS1:-$ }'\n",
    );
    script.push_str("tfy-human-bypass() {\n  TFY_HUMAN_BYPASS=1 PATH=\"${TFY_HUMAN_ORIGINAL_PATH:-$PATH}\" command \"$@\"\n  local status=$?\n  export TFY_LAST_STATUS=$status\n  return $status\n}\n");
    if auto_intercept {
        script.push_str(r##"export TFY_HUMAN_ORIGINAL_PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}"
export TFY_HUMAN_SHIM_DIR="${TFY_HUMAN_ROOT%/}/.tfy/human/bin"
	command -p mkdir -p "$TFY_HUMAN_SHIM_DIR" || return 1
	command -p chmod 700 "$TFY_HUMAN_SHIM_DIR" 2>/dev/null || true
_tfy_human_in_scope() {
  local cwd
  cwd="$(pwd -P 2>/dev/null)" || return 1
  case "$cwd/" in
    "${TFY_HUMAN_ROOT%/}/"*) return 0 ;;
    *) return 1 ;;
  esac
}
_tfy_human_is_excluded_name() {
  case "$1" in
    ""|tfy|command|builtin|source|.|eval|exec|alias|unalias|function|export|readonly|local|declare|typeset|set|unset|cd|pwd|return|exit|break|continue|shift|test|true|false|printf|echo|read|mapfile|type|hash|help|history|jobs|fg|bg|wait|trap|times|ulimit|umask|dirs|pushd|popd|compgen|complete|compopt|shopt|let|caller|bind|enable|logout|suspend|sh|bash|dash|zsh|fish|ksh|csh|tcsh|basename|cat|chmod|mkdir|mv|rm|tfy-human-bypass|_tfy_human_refresh_shims|_tfy_human_prompt_command|_tfy_human_in_scope|_tfy_human_is_excluded_name|_tfy_human_owned_shim|_tfy_human_validate_shim_dir|_tfy_human_disable_shims) return 0 ;;
    *[!A-Za-z0-9._+-]*) return 0 ;;
    *) return 1 ;;
  esac
}
_tfy_human_owned_shim() {
  [ -f "$1" ] || return 1
  [ ! -L "$1" ] || return 1
  IFS= read -r first_line < "$1" || return 1
  IFS= read -r second_line < "$1" || true
  [ "$first_line" = "#!/bin/sh" ] && [ "$second_line" = "# TFY:HUMAN-SHIM:v1" ]
}
_tfy_human_validate_shim_dir() {
  local entry
  for entry in "$TFY_HUMAN_SHIM_DIR"/*; do
    [ -e "$entry" ] || continue
    [ -f "$entry" ] || [ -L "$entry" ] || continue
    [ -x "$entry" ] || continue
    _tfy_human_owned_shim "$entry" || { printf 'tfy human shim refused non-TFY-owned path: %s\n' "$entry" >&2; return 1; }
  done
}
_tfy_human_disable_shims() {
  export TFY_HUMAN_AUTO_INTERCEPT=0
  local old_ifs part new_path
  old_ifs=$IFS
  IFS=:
  new_path=
  for part in $PATH; do
    [ "$part" = "$TFY_HUMAN_SHIM_DIR" ] && continue
    if [ -z "$new_path" ]; then
      new_path=$part
    else
      new_path="$new_path:$part"
    fi
  done
  IFS=$old_ifs
  export PATH="$new_path"
}
_tfy_human_write_shim() {
  local name="$1"
  local target="$TFY_HUMAN_SHIM_DIR/$name"
  local tmp="$TFY_HUMAN_SHIM_DIR/.$name.tmp.$$"
  if [ -e "$target" ] && ! _tfy_human_owned_shim "$target"; then
    printf 'tfy human shim refused non-TFY-owned path: %s\n' "$target" >&2
    return 1
  fi
  command -p cat > "$tmp" <<'TFY_HUMAN_SHIM'
#!/bin/sh
# TFY:HUMAN-SHIM:v1
cmd=${0##*/}
case "${TFY_HUMAN_BYPASS:-}" in
  1) PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}" exec "$cmd" "$@" ;;
esac
cwd=$(pwd -P 2>/dev/null || pwd)
case "$cwd/" in
  "${TFY_HUMAN_ROOT%/}/"*) ;;
  *) PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}" exec "$cmd" "$@" ;;
esac
PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}" \
TFY_HUMAN_BYPASS=1 \
exec tfy human run --session "${TFY_HUMAN_SESSION:-human}" --raw-dir "${TFY_HUMAN_RAW_DIR}" --ledger "${TFY_HUMAN_LEDGER}" -- "$cmd" "$@"
TFY_HUMAN_SHIM
  command -p chmod 755 "$tmp" 2>/dev/null || { command -p rm -f "$tmp"; return 1; }
  command -p mv -f "$tmp" "$target" || { command -p rm -f "$tmp"; return 1; }
}
_tfy_human_refresh_shims() {
  [ "${TFY_HUMAN_AUTO_INTERCEPT:-0}" = "1" ] || return 0
  [ -n "${TFY_HUMAN_ORIGINAL_PATH:-}" ] || return 0
  command -p mkdir -p "$TFY_HUMAN_SHIM_DIR" 2>/dev/null || return 1
  command -p chmod 700 "$TFY_HUMAN_SHIM_DIR" 2>/dev/null || true
  _tfy_human_validate_shim_dir || return 1
  local old_ifs dir entry name
  old_ifs=$IFS
  IFS=:
  for dir in $TFY_HUMAN_ORIGINAL_PATH; do
    [ -n "$dir" ] || continue
    [ "$dir" = "$TFY_HUMAN_SHIM_DIR" ] && continue
    [ -d "$dir" ] || continue
    for entry in "$dir"/*; do
      [ -e "$entry" ] || continue
      [ -f "$entry" ] || [ -L "$entry" ] || continue
      [ -x "$entry" ] || continue
      name=${entry##*/}
      _tfy_human_is_excluded_name "$name" && continue
      if [ -e "$TFY_HUMAN_SHIM_DIR/$name" ]; then
        _tfy_human_owned_shim "$TFY_HUMAN_SHIM_DIR/$name" || { IFS=$old_ifs; printf 'tfy human shim refused non-TFY-owned path: %s\n' "$TFY_HUMAN_SHIM_DIR/$name" >&2; return 1; }
      fi
      _tfy_human_write_shim "$name" || { IFS=$old_ifs; return 1; }
    done
  done
  IFS=$old_ifs
}
_tfy_human_prompt_command() {
  local status=$?
  export TFY_LAST_STATUS=$status
  _tfy_human_refresh_shims || { _tfy_human_disable_shims; return $status; }
  return $status
}
_tfy_human_refresh_shims || return $?
case ":$PATH:" in
  *":$TFY_HUMAN_SHIM_DIR:"*) ;;
  *) export PATH="$TFY_HUMAN_SHIM_DIR:$PATH" ;;
esac
case ";${PROMPT_COMMAND:-};" in
  *";_tfy_human_prompt_command;"*) ;;
  *) PROMPT_COMMAND="_tfy_human_prompt_command${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
esac
export PROMPT_COMMAND
"##);
    }
    script.push_str(END_MARKER);
    script.push('\n');
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_script_respects_no_auto_intercept() {
        let script = human_script(
            "human",
            Path::new(".tfy/raw"),
            Path::new(".tfy/human/ledger.jsonl"),
            "bash",
            false,
        )
        .expect("script");

        assert!(script.contains("TFY_HUMAN_AUTO_INTERCEPT=0"));
        assert!(script.contains("tfy-human-bypass()"));
        assert!(!script.contains("_tfy_human_run()"));
        assert!(!script.contains("git() {"));
    }
}
