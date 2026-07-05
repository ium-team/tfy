use crate::gateways::execute_structured_tool_gateway_with_origin;
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tfy_runtime::{AdapterKind, Origin, RouteEvidence};

const START_MARKER: &str = "# TFY:HUMAN-SESSION:START";
const END_MARKER: &str = "# TFY:HUMAN-SESSION:END";
const AUTO_START_MARKER: &str = "# TFY:HUMAN-AUTO-ACTIVATE:START";
const AUTO_END_MARKER: &str = "# TFY:HUMAN-AUTO-ACTIVATE:END";
const AUTO_SCRIPT_START_MARKER: &str = "# TFY:HUMAN-AUTO-SCRIPT:START";
const AUTO_SCRIPT_END_MARKER: &str = "# TFY:HUMAN-AUTO-SCRIPT:END";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HumanShellKind {
    Bash,
    Zsh,
    PowerShell,
}

impl HumanShellKind {
    pub(crate) fn parse(shell: &str) -> Result<Self> {
        match shell.trim().to_ascii_lowercase().as_str() {
            "auto" | "default" => Ok(Self::platform_default()),
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "powershell" | "powershell.exe" => Ok(Self::PowerShell),
            "pwsh" => bail!("tfy human shell does not support PowerShell 7 pwsh in this milestone; use --shell powershell on Windows"),
            "cmd" | "cmd.exe" => bail!("tfy human shell does not support cmd.exe in this milestone; use --shell powershell on Windows"),
            other => bail!("unsupported TFY human shell: {other}; supported shells are auto, bash, zsh, powershell"),
        }
    }

    pub(crate) fn platform_default() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::PowerShell
        }
        #[cfg(target_os = "macos")]
        {
            Self::Zsh
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            Self::Bash
        }
    }

    pub(crate) fn canonical(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::PowerShell => "powershell",
        }
    }

    fn command(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::PowerShell => "powershell.exe",
        }
    }

    fn script_ext(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::PowerShell => "ps1",
        }
    }

    fn session_script_name(self) -> &'static str {
        match self {
            Self::Bash => "session.bashrc",
            Self::Zsh => "session.zshrc",
            Self::PowerShell => "session.ps1",
        }
    }

    pub(crate) fn supported_on_this_platform(self) -> bool {
        match self {
            Self::Bash => cfg!(target_os = "linux"),
            Self::Zsh => cfg!(target_os = "macos"),
            Self::PowerShell => cfg!(target_os = "windows"),
        }
    }

    fn support_status(self) -> &'static str {
        match self {
            Self::Bash => "linux_bash_interactive_non_login_selected_rcfile",
            Self::Zsh => "macos_zsh_interactive_zdotdir_zshrc",
            Self::PowerShell => "windows_powershell_current_user_current_host_profile",
        }
    }
}

pub(crate) fn default_human_shell_name() -> &'static str {
    HumanShellKind::platform_default().canonical()
}

pub(crate) fn human_shells_supported_on_this_platform() -> Vec<String> {
    [
        HumanShellKind::Bash,
        HumanShellKind::Zsh,
        HumanShellKind::PowerShell,
    ]
    .into_iter()
    .filter(|kind| kind.supported_on_this_platform())
    .map(|kind| format!("{}-{}", std::env::consts::OS, kind.canonical()))
    .collect()
}

pub(crate) fn human_managed_session_supported_on_this_platform() -> bool {
    HumanShellKind::platform_default().supported_on_this_platform()
}

#[derive(Subcommand)]
pub(crate) enum HumanAutoActivateCmd {
    /// Install a TFY-owned shell startup hook into one explicit rcfile.
    Install(HumanAutoActivateInstallCmd),
    /// Remove the TFY-owned shell startup hook from one explicit rcfile.
    Uninstall(HumanAutoActivateUninstallCmd),
    /// Report current-directory marker, selected rcfile, and active shell auto-activation state.
    Status(HumanAutoActivateStatusCmd),
    /// Validate the current directory before an rc hook sources TFY activation content.
    Validate(HumanAutoActivateValidateCmd),
}

#[derive(Args)]
pub(crate) struct HumanAutoActivateInstallCmd {
    #[arg(long, default_value = "auto")]
    shell: String,
    #[arg(long)]
    rcfile: Option<PathBuf>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    apply: bool,
}

#[derive(Args)]
pub(crate) struct HumanAutoActivateUninstallCmd {
    #[arg(long, default_value = "auto")]
    shell: String,
    #[arg(long)]
    rcfile: Option<PathBuf>,
    #[arg(long)]
    apply: bool,
}

pub(crate) fn execute_human_auto_activate_install_setup(
    shell: &str,
    rcfile: Option<PathBuf>,
    dry_run: bool,
    apply: bool,
) -> Result<()> {
    execute_auto_activate_install(HumanAutoActivateInstallCmd {
        shell: shell.to_string(),
        rcfile,
        dry_run,
        apply,
    })
}

pub(crate) fn execute_human_auto_activate_uninstall_setup(
    shell: &str,
    rcfile: Option<PathBuf>,
    apply: bool,
) -> Result<()> {
    execute_auto_activate_uninstall(HumanAutoActivateUninstallCmd {
        shell: shell.to_string(),
        rcfile,
        apply,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct HumanAutoActivateHookStatus {
    pub(crate) shell: String,
    pub(crate) rcfile: PathBuf,
    pub(crate) hook_installed: bool,
    pub(crate) marker_block_present: bool,
}

pub(crate) fn human_auto_activate_hook_status(
    shell: &str,
    rcfile: Option<PathBuf>,
) -> Result<HumanAutoActivateHookStatus> {
    let kind = ensure_supported_shell(shell)?;
    let rcfile = rcfile.unwrap_or(home_rcfile(kind)?);
    let marker_block_present = match fs::read_to_string(&rcfile) {
        Ok(text) => text.contains(AUTO_START_MARKER) && text.contains(AUTO_END_MARKER),
        Err(_) => false,
    };
    Ok(HumanAutoActivateHookStatus {
        shell: kind.canonical().to_string(),
        rcfile,
        hook_installed: marker_block_present,
        marker_block_present,
    })
}

#[derive(Args)]
pub(crate) struct HumanAutoActivateStatusCmd {
    #[arg(long, default_value = "auto")]
    shell: String,
    #[arg(long)]
    rcfile: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
pub(crate) struct HumanAutoActivateValidateCmd {
    #[arg(long)]
    root: PathBuf,
    #[arg(long, default_value = "auto")]
    shell: String,
    #[arg(long)]
    quiet: bool,
}

#[derive(Subcommand)]
pub(crate) enum HumanCmd {
    /// Launch an explicit TFY-managed human shell session for the supported platform shell.
    Shell {
        #[arg(long, default_value = "human")]
        session: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value = ".tfy/human/ledger.jsonl")]
        ledger: PathBuf,
        #[arg(long, default_value = "auto")]
        shell: String,
        /// Disable PATH-shim auto-interception and keep only the managed shell environment.
        #[arg(long)]
        no_auto_intercept: bool,
    },
    /// Generate a sourceable integration script for a TFY-managed human session.
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
        #[arg(long, default_value = "auto")]
        shell: String,
    },
    /// Remove a TFY-owned generated human session script.
    Uninstall {
        #[arg(long)]
        path: PathBuf,
    },
    /// Manage current-directory future-shell auto-activation through an explicit shell rc/profile hook.
    AutoActivate {
        #[command(subcommand)]
        cmd: HumanAutoActivateCmd,
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
        HumanCmd::AutoActivate { cmd } => execute_human_auto_activate(cmd),
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
            AdapterKind::Shell,
            Origin::human_managed_session(),
            RouteEvidence::human_managed_session(),
        ),
    }
}

fn ensure_supported_shell(shell: &str) -> Result<HumanShellKind> {
    let kind = HumanShellKind::parse(shell)?;
    if !kind.supported_on_this_platform() {
        bail!(
            "tfy human shell does not support {} on {}; supported default here is {}",
            kind.canonical(),
            std::env::consts::OS,
            default_human_shell_name()
        );
    }
    Ok(kind)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HumanAutoActivateMarker {
    schema_version: u8,
    enabled: bool,
    shell: String,
    root: String,
    created_by: String,
    created_at: String,
    session: String,
    raw_dir: String,
    ledger: String,
    tfy_bin: String,
    script_sha256: String,
}

fn execute_human_auto_activate(cmd: HumanAutoActivateCmd) -> Result<()> {
    match cmd {
        HumanAutoActivateCmd::Install(cmd) => execute_auto_activate_install(cmd),
        HumanAutoActivateCmd::Uninstall(cmd) => execute_auto_activate_uninstall(cmd),
        HumanAutoActivateCmd::Status(cmd) => execute_auto_activate_status(cmd),
        HumanAutoActivateCmd::Validate(cmd) => execute_auto_activate_validate_cmd(cmd),
    }
}

fn canonical_tfy_exe() -> Result<PathBuf> {
    std::env::current_exe()
        .context("resolve current tfy executable")?
        .canonicalize()
        .context("canonicalize current tfy executable")
}

fn home_rcfile(kind: HumanShellKind) -> Result<PathBuf> {
    match kind {
        HumanShellKind::Bash => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| anyhow!("HOME is not set; pass --rcfile"))?;
            Ok(PathBuf::from(home).join(".bashrc"))
        }
        HumanShellKind::Zsh => {
            if let Some(zdotdir) = std::env::var_os("ZDOTDIR") {
                Ok(PathBuf::from(zdotdir).join(".zshrc"))
            } else {
                let home = std::env::var_os("HOME")
                    .ok_or_else(|| anyhow!("HOME is not set; pass --rcfile"))?;
                Ok(PathBuf::from(home).join(".zshrc"))
            }
        }
        HumanShellKind::PowerShell => powershell_profile_path(),
    }
}

fn powershell_profile_path() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        if let Some(userprofile) = std::env::var_os("USERPROFILE") {
            return Ok(PathBuf::from(userprofile)
                .join("Documents")
                .join("WindowsPowerShell")
                .join("Microsoft.PowerShell_profile.ps1"));
        }
    }
    bail!("could not resolve Windows PowerShell CurrentUserCurrentHost profile; pass --rcfile")
}

fn now_unix() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("unix:{seconds}")
}

fn ensure_not_symlink(path: &Path, label: &str) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("stat {label} {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refusing symlinked {label}: {}", path.display());
    }
    Ok(())
}

fn ensure_safe_regular_file(path: &Path, label: &str) -> Result<()> {
    ensure_not_symlink(path, label)?;
    let metadata =
        fs::metadata(path).with_context(|| format!("stat {label} {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{label} is not a regular file: {}", path.display());
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o022 != 0 {
        bail!("refusing group/world-writable {label}: {}", path.display());
    }
    Ok(())
}

fn ensure_safe_dir(path: &Path, label: &str) -> Result<()> {
    ensure_not_symlink(path, label)?;
    let metadata =
        fs::metadata(path).with_context(|| format!("stat {label} {}", path.display()))?;
    if !metadata.is_dir() {
        bail!("{label} is not a directory: {}", path.display());
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o022 != 0 {
        bail!("refusing group/world-writable {label}: {}", path.display());
    }
    Ok(())
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn auto_marker_path(root: &Path) -> PathBuf {
    root.join(".tfy/human/auto-activate.json")
}

fn auto_script_path(root: &Path, kind: HumanShellKind) -> PathBuf {
    root.join(format!(".tfy/human/auto-activate.{}", kind.script_ext()))
}

pub(crate) fn human_auto_script_relative_path(shell: &str) -> Result<String> {
    let kind = ensure_supported_shell(shell)?;
    Ok(format!(".tfy/human/auto-activate.{}", kind.script_ext()))
}

fn canonical_root(root: &Path) -> Result<PathBuf> {
    root.canonicalize()
        .with_context(|| format!("canonicalize root {}", root.display()))
}

fn ensure_repo_auto_dirs(root: &Path) -> Result<PathBuf> {
    let tfy_dir = root.join(".tfy");
    match fs::symlink_metadata(&tfy_dir) {
        Ok(_) => ensure_safe_dir(&tfy_dir, ".tfy")?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            fs::create_dir(&tfy_dir).with_context(|| format!("create {}", tfy_dir.display()))?;
            ensure_safe_dir(&tfy_dir, ".tfy")?;
        }
        Err(err) => return Err(err).with_context(|| format!("stat {}", tfy_dir.display())),
    }
    let human_dir = tfy_dir.join("human");
    match fs::symlink_metadata(&human_dir) {
        Ok(_) => ensure_safe_dir(&human_dir, ".tfy/human")?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            fs::create_dir(&human_dir)
                .with_context(|| format!("create {}", human_dir.display()))?;
            ensure_safe_dir(&human_dir, ".tfy/human")?;
        }
        Err(err) => return Err(err).with_context(|| format!("stat {}", human_dir.display())),
    }
    Ok(human_dir)
}

fn atomic_write_checked(path: &Path, text: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    ensure_safe_dir(parent, "write parent")?;
    if path.exists() {
        ensure_not_symlink(path, "target file")?;
    }
    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tfy"),
        std::process::id()
    ));
    if tmp.exists() {
        fs::remove_file(&tmp).with_context(|| format!("remove stale temp {}", tmp.display()))?;
    }
    fs::write(&tmp, text).with_context(|| format!("write {}", tmp.display()))?;
    replace_with_temp_file(path, &tmp)
}

fn replace_with_temp_file(path: &Path, tmp: &Path) -> Result<()> {
    if path.exists() {
        let backup = path.with_file_name(format!(
            ".{}.bak.{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("tfy"),
            std::process::id()
        ));
        if backup.exists() {
            fs::remove_file(&backup)
                .with_context(|| format!("remove stale backup {}", backup.display()))?;
        }
        fs::rename(path, &backup)
            .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        match fs::rename(tmp, path) {
            Ok(()) => {
                let _ = fs::remove_file(&backup);
                Ok(())
            }
            Err(err) => {
                let restore = fs::rename(&backup, path);
                if let Err(restore_err) = restore {
                    bail!(
                        "replace {} failed after backup and restore failed: replace_error={}; restore_error={}",
                        path.display(),
                        err,
                        restore_err
                    );
                }
                Err(err).with_context(|| format!("rename {} to {}", tmp.display(), path.display()))
            }
        }
    } else {
        fs::rename(tmp, path)
            .with_context(|| format!("rename {} to {}", tmp.display(), path.display()))
    }
}

fn ensure_rcfile_target_safe(path: &Path, must_exist: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_safe_dir(parent, "rcfile parent")?;
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                bail!("refusing symlinked rcfile: {}", path.display());
            }
            if !metadata.is_file() {
                bail!("rcfile is not a regular file: {}", path.display());
            }
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o022 != 0 {
                bail!("refusing group/world-writable rcfile: {}", path.display());
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound && !must_exist => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {
            bail!("rcfile does not exist: {}", path.display());
        }
        Err(err) => return Err(err).with_context(|| format!("stat {}", path.display())),
    }
    Ok(())
}

pub(crate) fn enable_human_auto_activate_repo(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    shell: &str,
    created_by: &str,
) -> Result<PathBuf> {
    let kind = ensure_supported_shell(shell)?;
    let root = canonical_root(Path::new("."))?;
    let tfy_bin = canonical_tfy_exe()?;
    if tfy_bin.starts_with(&root) {
        bail!(
            "refusing to pin repo-contained tfy executable {}; install or run a trusted external tfy binary",
            tfy_bin.display()
        );
    }
    ensure_repo_auto_dirs(&root)?;
    let script = auto_activation_script(session, raw_dir, ledger, kind, &tfy_bin)?;
    let marker = HumanAutoActivateMarker {
        schema_version: 1,
        enabled: true,
        shell: kind.canonical().into(),
        root: root.display().to_string(),
        created_by: created_by.into(),
        created_at: now_unix(),
        session: session.into(),
        raw_dir: raw_dir.display().to_string(),
        ledger: ledger.display().to_string(),
        tfy_bin: tfy_bin.display().to_string(),
        script_sha256: sha256_hex(&script),
    };
    write_auto_script(&auto_script_path(&root, kind), &script)?;
    let marker_text = format!("{}\n", serde_json::to_string_pretty(&marker)?);
    atomic_write_checked(&auto_marker_path(&root), &marker_text)?;
    validate_auto_activate_root(&root, kind.canonical())?;
    Ok(root)
}

fn auto_activation_script(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    kind: HumanShellKind,
    tfy_bin: &Path,
) -> Result<String> {
    let mut script = String::new();
    script.push_str(AUTO_SCRIPT_START_MARKER);
    script.push('\n');
    script.push_str("# TFY-generated current-directory human auto-activation script. Do not edit; regenerated by pinned TFY validation.\n");
    script.push_str(&human_script_with_tfy_bin(
        session, raw_dir, ledger, kind, true, tfy_bin,
    )?);
    script.push_str(AUTO_SCRIPT_END_MARKER);
    script.push('\n');
    Ok(script)
}

fn is_owned_auto_script(text: &str) -> bool {
    text.contains(AUTO_SCRIPT_START_MARKER) && text.contains(AUTO_SCRIPT_END_MARKER)
}

fn write_auto_script(path: &Path, script: &str) -> Result<()> {
    if path.exists() {
        ensure_not_symlink(path, "auto activation script")?;
        let existing =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        if !is_owned_auto_script(&existing) {
            bail!(
                "refusing to overwrite {}; missing TFY auto activation markers",
                path.display()
            );
        }
    }
    atomic_write_checked(path, script)?;
    Ok(())
}

fn read_marker(root: &Path) -> Result<HumanAutoActivateMarker> {
    let marker_path = auto_marker_path(root);
    ensure_safe_regular_file(&marker_path, "auto activation marker")?;
    let text = fs::read_to_string(&marker_path)
        .with_context(|| format!("read {}", marker_path.display()))?;
    let marker: HumanAutoActivateMarker =
        serde_json::from_str(&text).with_context(|| format!("parse {}", marker_path.display()))?;
    Ok(marker)
}

fn validate_auto_activate_root(root: &Path, shell: &str) -> Result<HumanAutoActivateMarker> {
    let kind = ensure_supported_shell(shell)?;
    let root = canonical_root(root)?;
    ensure_safe_dir(&root, "candidate root")?;
    ensure_safe_dir(&root.join(".tfy"), ".tfy")?;
    ensure_safe_dir(&root.join(".tfy/human"), ".tfy/human")?;
    let marker = read_marker(&root)?;
    if marker.schema_version != 1 {
        bail!(
            "unsupported auto activation marker schema_version={}",
            marker.schema_version
        );
    }
    if !marker.enabled {
        bail!("auto activation marker is disabled");
    }
    if marker.shell != kind.canonical() {
        bail!(
            "auto activation shell mismatch: marker={} requested={}",
            marker.shell,
            kind.canonical()
        );
    }
    if marker.root != root.to_string_lossy() {
        bail!(
            "auto activation root mismatch: marker={} candidate={}",
            marker.root,
            root.display()
        );
    }
    let tfy_bin = PathBuf::from(&marker.tfy_bin);
    if !tfy_bin.is_absolute() {
        bail!("auto activation marker tfy_bin is not absolute");
    }
    ensure_safe_regular_file(&tfy_bin, "pinned tfy executable")?;
    if tfy_bin.starts_with(&root) {
        bail!(
            "refusing repo-contained pinned tfy executable: {}",
            tfy_bin.display()
        );
    }
    let expected = auto_activation_script(
        &marker.session,
        Path::new(&marker.raw_dir),
        Path::new(&marker.ledger),
        kind,
        &tfy_bin,
    )?;
    if marker.script_sha256 != sha256_hex(&expected) {
        bail!("auto activation marker script hash does not match deterministic content");
    }
    let script_path = auto_script_path(&root, kind);
    match fs::read_to_string(&script_path) {
        Ok(existing) if existing == expected => {
            ensure_safe_regular_file(&script_path, "auto activation script")?;
        }
        Ok(existing) => {
            if !is_owned_auto_script(&existing) {
                bail!("auto activation script content mismatch and missing TFY ownership markers");
            }
            ensure_not_symlink(&script_path, "auto activation script")?;
            write_auto_script(&script_path, &expected)?;
            ensure_safe_regular_file(&script_path, "auto activation script")?;
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            write_auto_script(&script_path, &expected)?;
            ensure_safe_regular_file(&script_path, "auto activation script")?;
        }
        Err(err) => return Err(err).with_context(|| format!("read {}", script_path.display())),
    }
    Ok(marker)
}

fn execute_auto_activate_validate_cmd(cmd: HumanAutoActivateValidateCmd) -> Result<()> {
    match validate_auto_activate_root(&cmd.root, &cmd.shell) {
        Ok(marker) => {
            if !cmd.quiet {
                println!(
                    "valid=true root={} shell={} tfy_bin={}",
                    marker.root, marker.shell, marker.tfy_bin
                );
            }
            Ok(())
        }
        Err(err) => {
            if !cmd.quiet || std::env::var_os("TFY_HUMAN_AUTO_ACTIVATE_DEBUG").is_some() {
                eprintln!("tfy human auto-activate validate failed: {err:#}");
            }
            Err(err)
        }
    }
}

fn auto_hook_block(tfy_bin: &Path, kind: HumanShellKind) -> String {
    match kind {
        HumanShellKind::Bash => bash_auto_hook_block(tfy_bin),
        HumanShellKind::Zsh => zsh_auto_hook_block(tfy_bin),
        HumanShellKind::PowerShell => powershell_auto_hook_block(tfy_bin),
    }
}

fn bash_auto_hook_block(tfy_bin: &Path) -> String {
    let tfy_bin = shell_quote(&tfy_bin.display().to_string());
    format!(
        r#"{AUTO_START_MARKER}
# TFY-owned bash startup hook. It is silent unless TFY_HUMAN_AUTO_ACTIVATE_DEBUG=1.
_tfy_human_auto_activate() {{
  case "$-" in *i*) ;; *) return 0 ;; esac
  [ "${{TFY_DISABLE:-0}}" = "1" ] && return 0
  [ "${{TFY_HUMAN_AUTO_ACTIVATE_DISABLE:-0}}" = "1" ] && return 0
  [ "${{TFY_HUMAN_ACTIVE:-0}}" = "1" ] && return 0
  local TFY_HUMAN_AUTO_ACTIVATE_TFY={tfy_bin}
  local dir marker
  dir="$(pwd -P 2>/dev/null)" || return 0
  marker="$dir/.tfy/human/auto-activate.json"
  [ -f "$marker" ] || return 0
  if [ -L "$dir/.tfy" ] || [ -L "$marker" ]; then
    [ -n "${{TFY_HUMAN_AUTO_ACTIVATE_DEBUG:-}}" ] && printf 'tfy human auto-activate: refused symlink marker under %s\n' "$dir" >&2
    return 0
  fi
  if "$TFY_HUMAN_AUTO_ACTIVATE_TFY" human auto-activate validate --root "$dir" --shell bash --quiet >/dev/null 2>&1; then
    export TFY_HUMAN_ROOT="$dir"
    export TFY_HUMAN_TFY_BIN="$TFY_HUMAN_AUTO_ACTIVATE_TFY"
    # shellcheck source=/dev/null
    . "$TFY_HUMAN_ROOT/.tfy/human/auto-activate.bash"
    return $?
  else
    [ -n "${{TFY_HUMAN_AUTO_ACTIVATE_DEBUG:-}}" ] && printf 'tfy human auto-activate: validation failed under %s\n' "$dir" >&2
    return 0
  fi
}}
_tfy_human_auto_activate
unset -f _tfy_human_auto_activate 2>/dev/null || true
{AUTO_END_MARKER}
"#
    )
}

fn zsh_auto_hook_block(tfy_bin: &Path) -> String {
    let tfy_bin = shell_quote(&tfy_bin.display().to_string());
    format!(
        r#"{AUTO_START_MARKER}
# TFY-owned zsh startup hook. It is silent unless TFY_HUMAN_AUTO_ACTIVATE_DEBUG=1.
_tfy_human_auto_activate() {{
  case "$-" in *i*) ;; *) return 0 ;; esac
  [ "${{TFY_DISABLE:-0}}" = "1" ] && return 0
  [ "${{TFY_HUMAN_AUTO_ACTIVATE_DISABLE:-0}}" = "1" ] && return 0
  [ "${{TFY_HUMAN_ACTIVE:-0}}" = "1" ] && return 0
  local TFY_HUMAN_AUTO_ACTIVATE_TFY={tfy_bin}
  local dir marker
  dir="$(pwd -P 2>/dev/null)" || return 0
  marker="$dir/.tfy/human/auto-activate.json"
  [ -f "$marker" ] || return 0
  if [ -L "$dir/.tfy" ] || [ -L "$marker" ]; then
    [ -n "${{TFY_HUMAN_AUTO_ACTIVATE_DEBUG:-}}" ] && printf 'tfy human auto-activate: refused symlink marker under %s\n' "$dir" >&2
    return 0
  fi
  if "$TFY_HUMAN_AUTO_ACTIVATE_TFY" human auto-activate validate --root "$dir" --shell zsh --quiet >/dev/null 2>&1; then
    export TFY_HUMAN_ROOT="$dir"
    export TFY_HUMAN_TFY_BIN="$TFY_HUMAN_AUTO_ACTIVATE_TFY"
    . "$TFY_HUMAN_ROOT/.tfy/human/auto-activate.zsh"
    return $?
  else
    [ -n "${{TFY_HUMAN_AUTO_ACTIVATE_DEBUG:-}}" ] && printf 'tfy human auto-activate: validation failed under %s\n' "$dir" >&2
    return 0
  fi
}}
_tfy_human_auto_activate
unset -f _tfy_human_auto_activate 2>/dev/null || true
{AUTO_END_MARKER}
"#
    )
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn powershell_auto_hook_block(tfy_bin: &Path) -> String {
    let tfy_bin = powershell_quote(&tfy_bin.display().to_string());
    format!(
        r#"{AUTO_START_MARKER}
# TFY-owned PowerShell startup hook. Silent unless TFY_HUMAN_AUTO_ACTIVATE_DEBUG is set.
function Invoke-TfyHumanAutoActivate {{
  if ($env:TFY_DISABLE -eq '1' -or $env:TFY_HUMAN_AUTO_ACTIVATE_DISABLE -eq '1' -or $env:TFY_HUMAN_ACTIVE -eq '1') {{ return }}
  $tfy = {tfy_bin}
  $dir = (Get-Location).ProviderPath
  if (-not $dir) {{ return }}
  $marker = Join-Path $dir '.tfy/human/auto-activate.json'
  if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {{ return }}
  & $tfy human auto-activate validate --root $dir --shell powershell --quiet *> $null
  if ($LASTEXITCODE -eq 0) {{
    $env:TFY_HUMAN_ROOT = $dir
    $env:TFY_HUMAN_TFY_BIN = $tfy
    . (Join-Path $dir '.tfy/human/auto-activate.ps1')
  }} elseif ($env:TFY_HUMAN_AUTO_ACTIVATE_DEBUG) {{
    [Console]::Error.WriteLine("tfy human auto-activate: validation failed under $dir")
  }}
}}
Invoke-TfyHumanAutoActivate
Remove-Item Function:\Invoke-TfyHumanAutoActivate -ErrorAction SilentlyContinue
{AUTO_END_MARKER}
"#
    )
}

fn replace_marker_block(existing: &str, block: Option<&str>) -> Result<String> {
    let start = existing.find(AUTO_START_MARKER);
    let end = existing.find(AUTO_END_MARKER);
    match (start, end) {
        (Some(s), Some(e)) if s <= e => {
            let after = e + AUTO_END_MARKER.len();
            let mut out = String::new();
            out.push_str(&existing[..s]);
            if let Some(block) = block {
                out.push_str(block);
                if !block.ends_with('\n') {
                    out.push('\n');
                }
            }
            out.push_str(existing[after..].trim_start_matches('\n'));
            Ok(out)
        }
        (None, None) => {
            let mut out = existing.to_string();
            if let Some(block) = block {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(block);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Ok(out)
        }
        _ => bail!("malformed TFY human auto-activate marker block"),
    }
}

fn execute_auto_activate_install(cmd: HumanAutoActivateInstallCmd) -> Result<()> {
    let kind = ensure_supported_shell(&cmd.shell)?;
    if cmd.apply && cmd.dry_run {
        bail!("--apply and --dry-run cannot be combined");
    }
    let rcfile = cmd.rcfile.unwrap_or(home_rcfile(kind)?);
    ensure_rcfile_target_safe(&rcfile, false)?;
    let tfy_bin = canonical_tfy_exe()?;
    if tfy_bin.starts_with(canonical_root(Path::new("."))?) {
        bail!("refusing to pin repo-contained tfy executable {}; install or run a trusted external tfy binary", tfy_bin.display());
    }
    let block = auto_hook_block(&tfy_bin, kind);
    let existing = match fs::read_to_string(&rcfile) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("read {}", rcfile.display())),
    };
    let updated = replace_marker_block(&existing, Some(&block))?;
    if !cmd.apply {
        println!("TFY human auto-activate install dry-run");
        println!(
            "shell={} status=supported selected_rcfile.path={}",
            kind.canonical(),
            rcfile.display()
        );
        println!("pinned_tfy_bin={}", tfy_bin.display());
        println!("apply=false next_action=rerun with --apply");
        println!("block:\n{block}");
        return Ok(());
    }
    atomic_write_checked(&rcfile, &updated)?;
    println!(
        "installed TFY human auto-activate hook rcfile={} shell={} pinned_tfy_bin={}",
        rcfile.display(),
        kind.canonical(),
        tfy_bin.display()
    );
    Ok(())
}

fn execute_auto_activate_uninstall(cmd: HumanAutoActivateUninstallCmd) -> Result<()> {
    let kind = ensure_supported_shell(&cmd.shell)?;
    let rcfile = cmd.rcfile.unwrap_or(home_rcfile(kind)?);
    ensure_rcfile_target_safe(&rcfile, true)?;
    let existing =
        fs::read_to_string(&rcfile).with_context(|| format!("read {}", rcfile.display()))?;
    let updated = replace_marker_block(&existing, None)?;
    if !cmd.apply {
        println!("TFY human auto-activate uninstall dry-run");
        println!(
            "shell={} selected_rcfile.path={} apply=false next_action=rerun with --apply",
            kind.canonical(),
            rcfile.display()
        );
        return Ok(());
    }
    atomic_write_checked(&rcfile, &updated)?;
    println!(
        "removed TFY human auto-activate hook rcfile={}",
        rcfile.display()
    );
    Ok(())
}

fn execute_auto_activate_status(cmd: HumanAutoActivateStatusCmd) -> Result<()> {
    let kind = ensure_supported_shell(&cmd.shell)?;
    let root = canonical_root(Path::new(".")).ok();
    let marker_status = root.as_ref().and_then(|r| read_marker(r).ok());
    let marker_valid = root
        .as_ref()
        .map(|r| validate_auto_activate_root(r, &cmd.shell).is_ok())
        .unwrap_or(false);
    let hook_status = human_auto_activate_hook_status(&cmd.shell, cmd.rcfile.clone()).ok();
    let active_root = std::env::var("TFY_HUMAN_ROOT").ok();
    let report = json!({
        "repo_marker": {
            "enabled": marker_status.as_ref().map(|m| m.enabled).unwrap_or(false),
            "root": marker_status.as_ref().map(|m| m.root.clone()).or_else(|| root.as_ref().map(|r| r.display().to_string())),
            "valid": marker_valid
        },
        "selected_rcfile": {
            "path": hook_status.as_ref().map(|s| s.rcfile.display().to_string()),
            "shell": hook_status.as_ref().map(|s| s.shell.clone()).unwrap_or_else(|| kind.canonical().to_string()),
            "hook_installed": hook_status.as_ref().map(|s| s.hook_installed).unwrap_or(false),
            "marker_block_present": hook_status.as_ref().map(|s| s.marker_block_present).unwrap_or(false),
            "owned_block": hook_status.as_ref().map(|s| s.marker_block_present).unwrap_or(false)
        },
        "active_shell": {
            "managed": std::env::var("TFY_HUMAN_ACTIVE").ok().as_deref() == Some("1"),
            "root": active_root
        },
        "support_status": kind.support_status(),
        "shell": kind.canonical()
    });
    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "repo_marker.enabled={} repo_marker.valid={} repo_marker.root={}",
            report["repo_marker"]["enabled"],
            report["repo_marker"]["valid"],
            report["repo_marker"]["root"]
        );
        println!(
            "selected_rcfile.path={} selected_rcfile.hook_installed={} selected_rcfile.marker_block_present={} selected_rcfile.owned_block={}",
            report["selected_rcfile"]["path"],
            report["selected_rcfile"]["hook_installed"],
            report["selected_rcfile"]["marker_block_present"],
            report["selected_rcfile"]["owned_block"]
        );
        println!(
            "active_shell.managed={} active_shell.root={}",
            report["active_shell"]["managed"], report["active_shell"]["root"]
        );
        println!("support_status={}", kind.support_status());
    }
    Ok(())
}

pub(crate) fn execute_human_shell(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    shell: &str,
    auto_intercept: bool,
) -> Result<()> {
    let kind = ensure_supported_shell(shell)?;
    let integration_dir = PathBuf::from(".tfy/human");
    fs::create_dir_all(&integration_dir).context("create .tfy/human")?;
    let rcfile = integration_dir.join(kind.session_script_name());
    write_owned_script(
        &rcfile,
        &human_script(session, raw_dir, ledger, kind.canonical(), auto_intercept)?,
    )?;
    let root = std::env::current_dir()?
        .canonicalize()?
        .display()
        .to_string();
    let mut command = Command::new(kind.command());
    match kind {
        HumanShellKind::Bash => {
            command.arg("--rcfile").arg(&rcfile).arg("-i");
        }
        HumanShellKind::Zsh => {
            let zdotdir = integration_dir.join("zshdot");
            fs::create_dir_all(&zdotdir).context("create .tfy/human/zshdot")?;
            let zshrc = zdotdir.join(".zshrc");
            write_owned_script(&zshrc, &fs::read_to_string(&rcfile)?)?;
            command.arg("-i").env("ZDOTDIR", &zdotdir);
        }
        HumanShellKind::PowerShell => {
            command
                .arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NoExit")
                .arg("-Command")
                .arg(format!(
                    ". {}",
                    powershell_quote(&rcfile.display().to_string())
                ));
        }
    }
    let status = command
        .env("TFY_HUMAN_ACTIVE", "1")
        .env("TFY_HUMAN_SESSION", session)
        .env("TFY_HUMAN_RAW_DIR", raw_dir)
        .env("TFY_HUMAN_LEDGER", ledger)
        .env("TFY_HUMAN_ROOT", root)
        .env(
            "TFY_HUMAN_AUTO_INTERCEPT",
            if auto_intercept { "1" } else { "0" },
        )
        .status()
        .with_context(|| format!("launch {} for TFY human session", kind.canonical()))?;
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
    let kind = ensure_supported_shell(shell)?;
    let script = human_script(session, raw_dir, ledger, kind.canonical(), true)?;
    if dry_run || output.is_none() {
        let path = output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| format!(".tfy/human/{}", kind.session_script_name()));
        println!("TFY human install dry-run");
        println!(
            "shell={} status=supported scope=current_directory_scoped_tfy_managed_session",
            kind.canonical()
        );
        println!("would_write={path}");
        if kind == HumanShellKind::PowerShell {
            println!("source_command=. {}", powershell_quote(&path));
        } else {
            println!("source_command=source {}", shell_quote(&path));
        }
        println!("script:\n{script}");
        return Ok(());
    }
    let output = output.expect("checked above");
    write_owned_script(&output, &script)?;
    println!("installed TFY human session script at {}", output.display());
    println!(
        "shell={} status=supported scope=current_directory_scoped_tfy_managed_session",
        kind.canonical()
    );
    if kind == HumanShellKind::PowerShell {
        println!(
            "source it with: . {}",
            powershell_quote(&output.display().to_string())
        );
    } else {
        println!(
            "source it with: source {}",
            shell_quote(&output.display().to_string())
        );
    }
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
    shell: &str,
    auto_intercept: bool,
) -> Result<String> {
    let kind = ensure_supported_shell(shell)?;
    human_script_with_tfy_bin(
        session,
        raw_dir,
        ledger,
        kind,
        auto_intercept,
        &canonical_tfy_exe()?,
    )
}

fn powershell_human_script(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    auto_intercept: bool,
    tfy_bin: &Path,
) -> Result<String> {
    let session = powershell_quote(session);
    let raw_dir = powershell_quote(&raw_dir.display().to_string());
    let ledger = powershell_quote(&ledger.display().to_string());
    let tfy_bin = powershell_quote(&tfy_bin.display().to_string());
    let auto = if auto_intercept { "1" } else { "0" };
    let mut script = String::new();
    script.push_str(START_MARKER);
    script.push('\n');
    script.push_str("# TFY-managed PowerShell human integration. Source only where you want TFY command summaries.\n");
    script.push_str("# This is not global terminal interception; it uses TFY-owned proxy functions inside this managed PowerShell session.\n");
    script.push_str(&format!(
        "$env:TFY_HUMAN_ACTIVE = '1'\n$env:TFY_HUMAN_SESSION = {}\n$env:TFY_HUMAN_TFY_BIN = if ($env:TFY_HUMAN_TFY_BIN) {{ $env:TFY_HUMAN_TFY_BIN }} else {{ {} }}\n",
        session, tfy_bin
    ));
    script.push_str(
        r#"if (-not $env:TFY_HUMAN_ROOT) { $env:TFY_HUMAN_ROOT = (Get-Location).ProviderPath }
$env:TFY_HUMAN_ROOT = [System.IO.Path]::GetFullPath($env:TFY_HUMAN_ROOT)
"#,
    );
    script.push_str(&format!(
        "$tfyRawInput = {}\nif ([System.IO.Path]::IsPathRooted($tfyRawInput)) {{ $env:TFY_HUMAN_RAW_DIR = $tfyRawInput }} else {{ $env:TFY_HUMAN_RAW_DIR = Join-Path $env:TFY_HUMAN_ROOT $tfyRawInput }}\n",
        raw_dir
    ));
    script.push_str(&format!(
        "$tfyLedgerInput = {}\nif ([System.IO.Path]::IsPathRooted($tfyLedgerInput)) {{ $env:TFY_HUMAN_LEDGER = $tfyLedgerInput }} else {{ $env:TFY_HUMAN_LEDGER = Join-Path $env:TFY_HUMAN_ROOT $tfyLedgerInput }}\n",
        ledger
    ));
    script.push_str(&format!("$env:TFY_HUMAN_AUTO_INTERCEPT = '{}'\n", auto));
    script.push_str(r#"$global:TFY_LAST_STATUS = 0
function global:tfy-human-bypass {
  $oldBypass = $env:TFY_HUMAN_BYPASS
  $env:TFY_HUMAN_BYPASS = '1'
  try { & $args[0] @($args | Select-Object -Skip 1); $global:TFY_LAST_STATUS = $LASTEXITCODE } finally { $env:TFY_HUMAN_BYPASS = $oldBypass }
}
"#);
    if auto_intercept {
        script.push_str(r#"$global:TFY_HUMAN_ORIGINAL_COMMANDS = @{}
function global:Register-TfyHumanProxy {
  param([string] $Name, [string] $Path)
  if (-not $Name -or $Name -match '[^A-Za-z0-9._+-]') { return }
  switch ($Name) {
    'tfy' { return }
    'pwsh' { return }
    'powershell' { return }
    'powershell.exe' { return }
    'cmd' { return }
    'cmd.exe' { return }
    'tfy-human-bypass' { return }
  }
  $all = @(Get-Command -All $Name -ErrorAction SilentlyContinue)
  foreach ($cmd in $all) {
    if ($cmd.CommandType -ne 'Application') { return }
  }
  if (-not $global:TFY_HUMAN_ORIGINAL_COMMANDS.ContainsKey($Name)) {
    $global:TFY_HUMAN_ORIGINAL_COMMANDS[$Name] = $Path
  }
  $fn = @"
`$name = '$Name'
`$orig = `$global:TFY_HUMAN_ORIGINAL_COMMANDS[`$name]
if (`$env:TFY_HUMAN_BYPASS -eq '1') { & `$orig @args; return }
`$cwd = [System.IO.Path]::GetFullPath((Get-Location).ProviderPath)
`$root = [System.IO.Path]::GetFullPath(`$env:TFY_HUMAN_ROOT)
if (`$cwd -ne `$root) { & `$orig @args; return }
`$oldBypass = `$env:TFY_HUMAN_BYPASS
`$env:TFY_HUMAN_BYPASS = '1'
try {
  & `$env:TFY_HUMAN_TFY_BIN human run --session `$env:TFY_HUMAN_SESSION --raw-dir `$env:TFY_HUMAN_RAW_DIR --ledger `$env:TFY_HUMAN_LEDGER -- `$orig @args
  `$global:TFY_LAST_STATUS = `$LASTEXITCODE
  return
} finally {
  `$env:TFY_HUMAN_BYPASS = `$oldBypass
}
"@
  Set-Item -Path "Function:\global:$Name" -Value ([scriptblock]::Create($fn)) -Options AllScope
}
function global:Update-TfyHumanProxies {
  if ($env:TFY_HUMAN_AUTO_INTERCEPT -ne '1') { return }
  $apps = Get-Command -CommandType Application -ErrorAction SilentlyContinue
  foreach ($app in $apps) {
    Register-TfyHumanProxy -Name $app.Name -Path $app.Source
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($app.Name)
    if ($baseName -and $baseName -ne $app.Name) {
      Register-TfyHumanProxy -Name $baseName -Path $app.Source
    }
  }
}
Update-TfyHumanProxies
if (-not (Test-Path Function:\global:TFY_HUMAN_ORIGINAL_PROMPT)) {
  if (Test-Path Function:\global:prompt) {
    Set-Item -Path Function:\global:TFY_HUMAN_ORIGINAL_PROMPT -Value (Get-Item Function:\global:prompt).ScriptBlock -Options AllScope
  }
}
function global:prompt {
  Update-TfyHumanProxies
  if (Test-Path Function:\global:TFY_HUMAN_ORIGINAL_PROMPT) {
    & (Get-Item Function:\global:TFY_HUMAN_ORIGINAL_PROMPT).ScriptBlock
  } else {
    "PS $($executionContext.SessionState.Path.CurrentLocation)> "
  }
}
"#);
    }
    script.push_str(END_MARKER);
    script.push('\n');
    Ok(script)
}

fn human_script_with_tfy_bin(
    session: &str,
    raw_dir: &Path,
    ledger: &Path,
    kind: HumanShellKind,
    auto_intercept: bool,
    tfy_bin: &Path,
) -> Result<String> {
    if kind == HumanShellKind::PowerShell {
        return powershell_human_script(session, raw_dir, ledger, auto_intercept, tfy_bin);
    }

    let session = shell_quote(session);
    let raw_dir = shell_quote(&raw_dir.display().to_string());
    let ledger = shell_quote(&ledger.display().to_string());
    let tfy_bin = shell_quote(&tfy_bin.display().to_string());
    let mut script = String::new();
    script.push_str(START_MARKER);
    script.push('\n');
    script.push_str("# TFY-managed human shell integration. Source only in shells where you want TFY command summaries.\n");
    script.push_str("# This is not global terminal interception; it routes PATH-resolved external commands only inside this managed shell session.\n");
    script.push_str(&format!(
        "export TFY_HUMAN_ACTIVE=1\nexport TFY_HUMAN_SESSION={}\n",
        session
    ));
    script.push_str("TFY_HUMAN_ROOT=\"$(cd -P -- \"${TFY_HUMAN_ROOT:-$PWD}\" 2>/dev/null && pwd -P)\" || TFY_HUMAN_ROOT=\"${TFY_HUMAN_ROOT:-$PWD}\"\nexport TFY_HUMAN_ROOT\n");
    script.push_str(&format!(
        "TFY_HUMAN_TFY_BIN=\"${{TFY_HUMAN_TFY_BIN:-{}}}\"\nexport TFY_HUMAN_TFY_BIN\n",
        tfy_bin
    ));
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
        script.push_str(r##"export TFY_HUMAN_SHIM_DIR="${TFY_HUMAN_ROOT%/}/.tfy/human/bin"
if [ -n "${ZSH_VERSION:-}" ]; then
  setopt NULL_GLOB 2>/dev/null || true
fi
_tfy_human_strip_shim_from_path() {
  local input_path="$1"
  local old_ifs part new_path
  old_ifs=$IFS
  IFS=:
  new_path=
  for part in $input_path; do
    [ "$part" = "$TFY_HUMAN_SHIM_DIR" ] && continue
    if [ -z "$new_path" ]; then
      new_path=$part
    else
      new_path="$new_path:$part"
    fi
  done
  IFS=$old_ifs
  printf '%s' "$new_path"
}
if [ -n "${TFY_HUMAN_ORIGINAL_PATH:-}" ]; then
  export TFY_HUMAN_ORIGINAL_PATH="$(_tfy_human_strip_shim_from_path "$TFY_HUMAN_ORIGINAL_PATH")"
else
  export TFY_HUMAN_ORIGINAL_PATH="$(_tfy_human_strip_shim_from_path "$PATH")"
fi
	command -p mkdir -p "$TFY_HUMAN_SHIM_DIR" || return 1
	command -p chmod 700 "$TFY_HUMAN_SHIM_DIR" 2>/dev/null || true
_tfy_human_in_scope() {
  local cwd
  cwd="$(pwd -P 2>/dev/null)" || return 1
  [ "$cwd" = "$TFY_HUMAN_ROOT" ]
}
_tfy_human_is_excluded_name() {
  case "$1" in
    ""|tfy|command|builtin|source|.|eval|exec|alias|unalias|function|export|readonly|local|declare|typeset|set|unset|cd|pwd|return|exit|break|continue|shift|test|true|false|printf|echo|read|mapfile|type|hash|help|history|jobs|fg|bg|wait|trap|times|ulimit|umask|dirs|pushd|popd|compgen|complete|compopt|shopt|let|caller|bind|enable|logout|suspend|sh|bash|dash|zsh|fish|ksh|csh|tcsh|basename|cat|chmod|mkdir|mv|rm|tfy-human-bypass|_tfy_human_refresh_shims|_tfy_human_prompt_command|_tfy_human_in_scope|_tfy_human_is_excluded_name|_tfy_human_owned_shim|_tfy_human_validate_shim_dir|_tfy_human_disable_shims|_tfy_human_strip_shim_from_path) return 0 ;;
    *[!A-Za-z0-9._+-]*) return 0 ;;
    *) return 1 ;;
  esac
}
_tfy_human_owned_shim() {
  [ -f "$1" ] || return 1
  [ ! -L "$1" ] || return 1
  {
    IFS= read -r first_line || return 1
    IFS= read -r second_line || true
  } < "$1"
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
  hash -r 2>/dev/null || true
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
[ "$cwd" = "$TFY_HUMAN_ROOT" ] || PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}" exec "$cmd" "$@"
PATH="${TFY_HUMAN_ORIGINAL_PATH:-$PATH}" \
TFY_HUMAN_BYPASS=1 \
exec "${TFY_HUMAN_TFY_BIN:?}" human run --session "${TFY_HUMAN_SESSION:-human}" --raw-dir "${TFY_HUMAN_RAW_DIR}" --ledger "${TFY_HUMAN_LEDGER}" -- "$cmd" "$@"
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
hash -r 2>/dev/null || true
if [ -n "${ZSH_VERSION:-}" ]; then
  if ! whence -w _tfy_human_prompt_command >/dev/null 2>&1; then
    :
  fi
  if ! printf '%s\n' "${precmd_functions:-}" | grep -qx '_tfy_human_prompt_command' 2>/dev/null; then
    eval 'precmd_functions=(_tfy_human_prompt_command ${precmd_functions[@]})'
  fi
else
  case ";${PROMPT_COMMAND:-};" in
    *";_tfy_human_prompt_command;"*) ;;
    *) PROMPT_COMMAND="_tfy_human_prompt_command${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
  esac
  export PROMPT_COMMAND
fi
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
            default_human_shell_name(),
            false,
        )
        .expect("script");

        assert!(script.contains("TFY_HUMAN_AUTO_INTERCEPT=0"));
        assert!(script.contains("tfy-human-bypass"));
        assert!(!script.contains("_tfy_human_run()"));
        assert!(!script.contains("git() {"));
    }

    #[test]
    fn powershell_script_uses_windows_powershell_proxy_contract() {
        assert_eq!(HumanShellKind::PowerShell.command(), "powershell.exe");
        let script = powershell_human_script(
            "human",
            Path::new(".tfy/raw"),
            Path::new(".tfy/human/ledger.jsonl"),
            true,
            Path::new("C:/tfy/tfy.exe"),
        )
        .expect("script");

        assert!(script.contains("function global:tfy-human-bypass"));
        assert!(script.contains("function global:Register-TfyHumanProxy"));
        assert!(script.contains("function global:Update-TfyHumanProxies"));
        assert!(script.contains("Get-Command -All $Name"));
        assert!(script.contains("GetFileNameWithoutExtension($app.Name)"));
        assert!(script.contains("Register-TfyHumanProxy -Name $baseName -Path $app.Source"));
        assert!(script.contains(r"Function:\global:TFY_HUMAN_ORIGINAL_PROMPT"));
        assert!(script.contains("function global:prompt"));
        assert!(script.contains("TFY_HUMAN_TFY_BIN human run"));
    }
}
