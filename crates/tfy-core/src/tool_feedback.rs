use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSummary {
    pub command: String,
    pub exit_code: i32,
    pub risk: String,
    pub summary: String,
    pub model_text: String,
    pub rendering_kind: String,
    pub raw_ref: String,
    pub raw_chars: usize,
    pub summary_chars: usize,
    pub savings_pct: f64,
    pub evidence: Vec<String>,
    pub command_family: String,
    pub strategy_kind: String,
    pub human_auto_safe: bool,
    pub agent_safe: bool,
    pub interactive_risk: String,
    pub output_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandStrategyMetadata {
    pub family: String,
    pub strategy_kind: String,
    pub human_auto_safe: bool,
    pub agent_safe: bool,
    pub streaming_safe: bool,
    pub interactive_risk: String,
    pub claim_status: String,
}

#[derive(Debug, Clone, Copy)]
struct CommandStrategySpec {
    family: &'static str,
    strategy_kind: &'static str,
    human_auto_safe: bool,
    agent_safe: bool,
    streaming_safe: bool,
    interactive_risk: &'static str,
    claim_status: &'static str,
}

const RUST_STRATEGY_FAMILIES: &[&str] = &[
    "git_status",
    "git_diff",
    "git_log",
    "gh_pr_checks",
    "cargo_test",
    "cargo_clippy",
    "cargo_build",
    "cargo_check",
    "cargo_fmt_check",
    "tsc_check",
    "pytest",
    "npm_test",
    "pnpm_test",
    "yarn_test",
    "go_test",
    "maven_test",
    "gradle_test",
];

const GENERIC_STRATEGY_SPEC: CommandStrategySpec = CommandStrategySpec {
    family: "generic",
    strategy_kind: "generic",
    human_auto_safe: false,
    agent_safe: true,
    streaming_safe: false,
    interactive_risk: "unknown",
    claim_status: "generic_fallback",
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandStrategyRegistry;

impl CommandStrategyRegistry {
    pub fn classify(&self, command: &str) -> CommandStrategyMetadata {
        self.metadata_for_family(&classify_command_family(command))
    }

    pub fn metadata_for_family(&self, family: &str) -> CommandStrategyMetadata {
        if RUST_STRATEGY_FAMILIES.contains(&family) {
            return CommandStrategyMetadata {
                family: family.to_string(),
                strategy_kind: "rust".to_string(),
                human_auto_safe: false,
                agent_safe: true,
                streaming_safe: false,
                interactive_risk: "none".to_string(),
                claim_status: "implemented_p0_not_comparison_claimed".to_string(),
            };
        }
        if let Some(filter) = built_in_filter_for_family(family) {
            return CommandStrategyMetadata {
                family: filter.family.to_string(),
                strategy_kind: "dsl".to_string(),
                human_auto_safe: filter.human_auto_safe,
                agent_safe: true,
                streaming_safe: false,
                interactive_risk: filter.interactive_risk.to_string(),
                claim_status: "built_in_filter_foundation".to_string(),
            };
        }
        CommandStrategyMetadata {
            family: GENERIC_STRATEGY_SPEC.family.to_string(),
            strategy_kind: GENERIC_STRATEGY_SPEC.strategy_kind.to_string(),
            human_auto_safe: GENERIC_STRATEGY_SPEC.human_auto_safe,
            agent_safe: GENERIC_STRATEGY_SPEC.agent_safe,
            streaming_safe: GENERIC_STRATEGY_SPEC.streaming_safe,
            interactive_risk: GENERIC_STRATEGY_SPEC.interactive_risk.to_string(),
            claim_status: GENERIC_STRATEGY_SPEC.claim_status.to_string(),
        }
    }

    fn summary_candidate(&self, input: StrategySummaryInput<'_>) -> Option<String> {
        rust_strategy_summary_candidate(
            input.family,
            input.cmd,
            input.code,
            input.raw,
            input.evidence,
            input.rr,
            input.risk,
        )
        .or_else(|| {
            built_in_filter_summary_candidate(
                input.family,
                input.cmd,
                input.code,
                input.raw,
                input.evidence,
                input.rr,
                input.risk,
            )
        })
    }
}

struct StrategySummaryInput<'a> {
    family: &'a str,
    cmd: &'a str,
    code: i32,
    raw: &'a str,
    evidence: &'a [String],
    rr: &'a str,
    risk: &'a str,
}

pub fn command_strategy_metadata(family: &str) -> CommandStrategyMetadata {
    CommandStrategyRegistry.metadata_for_family(family)
}

pub fn classify_command_strategy(command: &str) -> CommandStrategyMetadata {
    CommandStrategyRegistry.classify(command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    Auto,
    Generic,
    GitGithub,
}

struct StoredRaw {
    raw_ref: String,
    output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct RawStore {
    root: PathBuf,
}

impl RawStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        Ok(Self { root })
    }
    pub fn put(&self, command: &str, raw: &str, exit_code: i32) -> Result<String> {
        self.put_bytes(command, raw.as_bytes(), exit_code)
    }
    pub fn put_bytes(&self, command: &str, raw: &[u8], exit_code: i32) -> Result<String> {
        for attempt in 0..16u64 {
            let nonce = now_ns().wrapping_add(attempt);
            let mut hasher = Sha256::new();
            hasher.update(command);
            hasher.update([0]);
            hasher.update(exit_code.to_string());
            hasher.update([0]);
            hasher.update(raw);
            hasher.update(nonce.to_string());
            let hex = format!("{:x}", hasher.finalize());
            let rf = format!("cmdout_{}_{nonce:016x}", &hex[..12]);
            let path = self.path_for(&rf)?;
            let raw_text = std::str::from_utf8(raw).ok();
            let payload = serde_json::json!({
                "command":command,
                "exit_code":exit_code,
                "raw":raw_text,
                "raw_b64":BASE64_STANDARD.encode(raw),
                "created_ns":nonce
            });
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            options.mode(0o600);
            match options.open(&path) {
                Ok(mut f) => {
                    f.write_all(serde_json::to_string_pretty(&payload)?.as_bytes())?;
                    return Ok(rf);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        bail!("could not allocate raw_ref")
    }
    fn path_for(&self, raw_ref: &str) -> Result<PathBuf> {
        let re = Regex::new(r"^cmdout_[0-9a-f]{12}_[0-9a-f]{16}$").expect("valid raw_ref regex");
        if !re.is_match(raw_ref) {
            bail!("invalid raw ref: {raw_ref}");
        }
        let root = self.root.canonicalize().or_else(|_| {
            fs::create_dir_all(&self.root)?;
            self.root.canonicalize()
        })?;
        let path = root.join(format!("{raw_ref}.json"));
        let Some(parent_path) = path.parent() else {
            bail!("raw ref path has no parent");
        };
        let parent = parent_path.canonicalize()?;
        if parent != root {
            bail!("raw ref escapes store");
        }
        Ok(path)
    }
    pub fn raw(&self, raw_ref: &str, around: Option<&str>, context: usize) -> Result<String> {
        let raw_bytes = self.raw_bytes(raw_ref, around, context)?;
        Ok(String::from_utf8_lossy(&raw_bytes).to_string())
    }
    pub fn raw_bytes(
        &self,
        raw_ref: &str,
        around: Option<&str>,
        context: usize,
    ) -> Result<Vec<u8>> {
        let path = self.path_for(raw_ref)?;
        let text = fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;
        let raw = if let Some(raw_b64) = v["raw_b64"].as_str() {
            BASE64_STANDARD.decode(raw_b64)?
        } else {
            v["raw"].as_str().unwrap_or("").as_bytes().to_vec()
        };
        if let Some(needle) = around {
            let raw_text = std::str::from_utf8(&raw)?;
            let lines: Vec<_> = raw_text.lines().collect();
            let mut out = Vec::new();
            for (i, _l) in lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.contains(needle))
                .take(5)
            {
                let start = i.saturating_sub(context);
                let end = (i + context + 1).min(lines.len());
                out.extend_from_slice(&lines[start..end]);
            }
            if !out.is_empty() {
                return Ok((out.join("\n") + "\n").into_bytes());
            }
            bail!("raw range needle not found: {needle}");
        }
        Ok(raw)
    }
}

pub fn run_command(
    command: &[String],
    cwd: Option<&Path>,
    raw_dir: impl AsRef<Path>,
    max_summary_bytes: usize,
) -> Result<CommandSummary> {
    let store = RawStore::new(raw_dir)?;
    if command.is_empty() {
        return compress(
            &store,
            "",
            "[tfy: command launch failed] empty command\n",
            127,
            Some(max_summary_bytes),
            ToolPolicy::Auto,
        );
    }
    let mut cmd = Command::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(out) => {
            let mut raw = out.stdout;
            raw.extend_from_slice(&out.stderr);
            compress_bytes(
                &store,
                &command.join(" "),
                &raw,
                out.status.code().unwrap_or(-1),
                Some(max_summary_bytes),
                ToolPolicy::Auto,
            )
        }
        Err(e) => compress(
            &store,
            &command.join(" "),
            &format!(
                "[tfy: command launch failed] {}: {}\n",
                std::any::type_name_of_val(&e),
                e
            ),
            127,
            Some(max_summary_bytes),
            ToolPolicy::Auto,
        ),
    }
}

pub fn raw_output(
    raw_dir: impl AsRef<Path>,
    raw_ref: &str,
    around: Option<&str>,
    context: usize,
) -> Result<String> {
    RawStore::new(raw_dir)?.raw(raw_ref, around, context)
}

pub fn raw_output_bytes(
    raw_dir: impl AsRef<Path>,
    raw_ref: &str,
    around: Option<&str>,
    context: usize,
) -> Result<Vec<u8>> {
    RawStore::new(raw_dir)?.raw_bytes(raw_ref, around, context)
}

pub fn summarize_command_output(
    command: &str,
    raw: &str,
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
) -> Result<CommandSummary> {
    summarize_command_output_with_policy(command, raw, exit_code, raw_dir, ToolPolicy::Auto)
}

pub fn summarize_command_output_with_policy(
    command: &str,
    raw: &str,
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
    policy: ToolPolicy,
) -> Result<CommandSummary> {
    let store = RawStore::new(raw_dir)?;
    compress(&store, command, raw, exit_code, None, policy)
}

fn compress(
    store: &RawStore,
    command: &str,
    raw: &str,
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
) -> Result<CommandSummary> {
    compress_bytes(
        store,
        command,
        raw.as_bytes(),
        exit_code,
        max_summary_bytes,
        requested_policy,
    )
}

fn compress_bytes(
    store: &RawStore,
    command: &str,
    raw_bytes: &[u8],
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
) -> Result<CommandSummary> {
    let raw_ref = store.put_bytes(command, raw_bytes, exit_code)?;
    let raw = String::from_utf8_lossy(raw_bytes);
    compress_with_raw_ref(
        command,
        &raw,
        raw_bytes.len(),
        exit_code,
        max_summary_bytes,
        requested_policy,
        StoredRaw {
            raw_ref,
            output_sha256: sha256_hex(raw_bytes),
        },
    )
}

fn compress_with_raw_ref(
    command: &str,
    raw: &str,
    raw_len: usize,
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
    stored_raw: StoredRaw,
) -> Result<CommandSummary> {
    let raw_ref = stored_raw.raw_ref;
    let output_sha256 = stored_raw.output_sha256;
    let policy = GitGithubToolPolicy::new();
    let registry = CommandStrategyRegistry;
    let strategy_metadata = registry.classify(command);
    let command_family = strategy_metadata.family.clone();
    let is_git_github = match requested_policy {
        ToolPolicy::Auto => policy.matches(command),
        ToolPolicy::Generic => false,
        ToolPolicy::GitGithub => true,
    };
    let evidence = if is_git_github {
        policy.evidence(command, raw)
    } else {
        evidence(raw)
    }
    .into_iter()
    .map(|line| redact_public(&line))
    .collect::<Vec<_>>();
    let risk = family_risk(
        &command_family,
        command,
        raw,
        exit_code,
        is_git_github,
        &policy,
    );
    let display_command = redact_public(command);
    let summary_candidate = registry
        .summary_candidate(StrategySummaryInput {
            family: &command_family,
            cmd: &display_command,
            code: exit_code,
            raw,
            evidence: &evidence,
            rr: &raw_ref,
            risk: &risk,
        })
        .unwrap_or_else(|| match risk.as_str() {
            "critical" => critical(&display_command, exit_code, &evidence, &raw_ref),
            "success" => success(&display_command, exit_code, raw, &raw_ref),
            _ => unknown(&display_command, exit_code, raw, &evidence, &raw_ref),
        });
    let summary_candidate =
        cap_summary_preserving_raw_ref(summary_candidate, &raw_ref, max_summary_bytes);
    let public_raw = public_raw_candidate(raw, &raw_ref);
    let decision = choose_model_visible_text(&summary_candidate, &public_raw);
    let savings_pct = savings_pct_floor(raw_len, decision.text.len());
    Ok(CommandSummary {
        command: display_command,
        exit_code,
        risk,
        summary_chars: decision.text.len(),
        raw_chars: raw_len,
        savings_pct,
        summary: decision.text.clone(),
        model_text: decision.text,
        rendering_kind: decision.kind,
        raw_ref,
        evidence,
        command_family,
        strategy_kind: strategy_metadata.strategy_kind,
        human_auto_safe: strategy_metadata.human_auto_safe,
        agent_safe: strategy_metadata.agent_safe,
        interactive_risk: strategy_metadata.interactive_risk,
        output_sha256,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn savings_pct_floor(raw_len: usize, model_len: usize) -> f64 {
    if raw_len == 0 || model_len >= raw_len {
        0.0
    } else {
        ((raw_len as f64 - model_len as f64) / raw_len as f64 * 10000.0).round() / 100.0
    }
}

struct ModelOutputDecision {
    text: String,
    kind: String,
}

fn choose_model_visible_text(
    summary_candidate: &str,
    public_raw: &PublicRawCandidate,
) -> ModelOutputDecision {
    if summary_candidate.len() < public_raw.text.len() {
        ModelOutputDecision {
            text: summary_candidate.to_string(),
            kind: "summary".to_string(),
        }
    } else if public_raw.suppressed {
        ModelOutputDecision {
            text: public_raw.text.clone(),
            kind: "suppressed".to_string(),
        }
    } else {
        ModelOutputDecision {
            text: public_raw.text.clone(),
            kind: "pass_through".to_string(),
        }
    }
}

struct PublicRawCandidate {
    text: String,
    suppressed: bool,
}

fn public_raw_candidate(raw: &str, raw_ref: &str) -> PublicRawCandidate {
    if is_suppressed_public_raw(raw) {
        return PublicRawCandidate {
            text: format!(
                "[tfy: output suppressed; unsafe or binary-ish content stored locally; raw_ref={raw_ref}]\n"
            ),
            suppressed: true,
        };
    }
    PublicRawCandidate {
        text: redact_public(raw),
        suppressed: false,
    }
}

fn is_suppressed_public_raw(raw: &str) -> bool {
    raw.chars()
        .any(|c| (c.is_control() && !matches!(c, '\n' | '\r' | '\t')) || c == '\u{fffd}')
}

struct GitGithubToolPolicy {
    command: Regex,
    git_status: Regex,
    git_data: Regex,
    gh_review_data: Regex,
    git_visible_data: Regex,
    git_error: Regex,
    negative: Regex,
    file_line: Regex,
    diff_hunk: Regex,
    url: Regex,
}

impl GitGithubToolPolicy {
    fn new() -> Self {
        let generic = r"error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL";
        let git = format!(
            r"{generic}|conflict|rejected|non-fast-forward|rate limit|authenticate|HTTP\s+[45]\d\d|changes_requested|unresolved"
        );
        Self {
            command: Regex::new(r"^(?:git|gh)(?:\s|$)").expect("valid command regex"),
            git_status: Regex::new(r"^git\s+status(?:\s|$)").expect("valid git status regex"),
            git_data: Regex::new(r"^git\s+(?:diff|show|log)(?:\s|$)").expect("valid git data regex"),
            gh_review_data: Regex::new(r"^gh\s+(?:pr|issue)\s+(?:view|status)(?:\s|$)").expect("valid gh review regex"),
            git_visible_data: Regex::new(r"^git\s+(?:diff|show|log|status)(?:\s|$)").expect("valid git visible regex"),
            git_error: Regex::new(&format!(r"(?i)({git})")).expect("valid git error regex"),
            negative: Regex::new(r"(?i)(not\s+successful|unsuccessful|failed|failing|cancel(?:led|ed)|timed?\s+out|action_required)").expect("valid negative regex"),
            file_line: Regex::new(r"[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md)(?::\d+)?").expect("valid file regex"),
            diff_hunk: Regex::new(r"^@@\s.*@@").expect("valid diff hunk regex"),
            url: Regex::new(r#"https?://[^\s\"'<>]+"#).expect("valid url regex"),
        }
    }

    fn matches(&self, command: &str) -> bool {
        self.command.is_match(command.trim())
    }

    fn risk(&self, command: &str, raw: &str, exit_code: i32) -> String {
        let output = raw.trim_matches(['\r', '\n']);
        if exit_code != 0 {
            return "critical".into();
        }
        if self.is_git_status(command) {
            if self.is_clean_git_status(output) {
                return "success".into();
            }
            if self.has_git_status_conflict_marker(output)
                || self.has_long_git_status_blocker(output)
            {
                return "critical".into();
            }
            return if output.is_empty() {
                "success"
            } else {
                "unknown"
            }
            .into();
        }
        if (self.is_git_data(command) || self.is_gh_review_data(command)) && !output.is_empty() {
            return "unknown".into();
        }
        if self.git_error.is_match(raw) || self.negative.is_match(raw) {
            return "critical".into();
        }
        if self.is_git_github_success(command, raw) {
            return "success".into();
        }
        "unknown".into()
    }

    fn is_git_status(&self, command: &str) -> bool {
        self.git_status.is_match(command.trim())
    }
    fn is_git_data(&self, command: &str) -> bool {
        self.git_data.is_match(command.trim())
    }
    fn is_gh_review_data(&self, command: &str) -> bool {
        self.gh_review_data.is_match(command.trim())
    }
    fn is_git_github_success(&self, command: &str, raw: &str) -> bool {
        let text = raw.trim();
        if text.is_empty() {
            return true;
        }
        if self.is_git_status(command) {
            return self.is_clean_git_status(text);
        }
        if self.git_visible_data.is_match(command.trim()) || self.negative.is_match(text) {
            return false;
        }
        Regex::new(
            r"(?i)all checks passed|checks? passed|success(?:ful)?|passed|up[ -]?to[ -]?date",
        )
        .expect("valid git success regex")
        .is_match(text)
    }
    fn is_clean_git_status(&self, text: &str) -> bool {
        if text.is_empty() {
            return true;
        }
        let normalized = text
            .trim()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        let lines: Vec<_> = normalized
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        if !lines.is_empty() && lines.iter().all(|line| line.starts_with("##")) {
            return true;
        }
        if matches!(
            normalized.as_str(),
            "nothing to commit, working tree clean" | "nothing to commit, working tree clean."
        ) {
            return true;
        }
        Regex::new(r"(?is)^On branch .+(?:\nYour branch is up to date with .+\.)?\n\nnothing to commit, working tree clean\.?$")
            .expect("valid clean status regex")
            .is_match(&normalized)
    }
    fn looks_like_git_status_line(&self, line: &str) -> bool {
        if line.starts_with("##") {
            return true;
        }
        let bytes = line.as_bytes();
        bytes.len() >= 3
            && b" MADRCUT?!".contains(&bytes[0])
            && b" MADRCUT?!".contains(&bytes[1])
            && bytes[2].is_ascii_whitespace()
    }
    fn has_git_status_conflict_marker(&self, text: &str) -> bool {
        text.lines().any(|line| {
            Regex::new(r"^(?:UU|AA|DD|AU|UA|DU|UD)(?:\s|$)")
                .expect("valid conflict marker regex")
                .is_match(line.trim())
        })
    }
    fn has_long_git_status_blocker(&self, text: &str) -> bool {
        text.lines()
            .any(|line| self.is_long_git_status_blocker_line(line))
    }
    fn is_long_git_status_blocker_line(&self, line: &str) -> bool {
        Regex::new(
            r"(?i)^(?:unmerged paths:|conflict\b|rejected\b|authentication\b|fatal\b|error\b)",
        )
        .expect("valid blocker regex")
        .is_match(line.trim())
    }
    fn git_status_line_evidence(&self, line: &str) -> Option<String> {
        if line.starts_with("##") || !self.looks_like_git_status_line(line) {
            return None;
        }
        let path = if line.len() > 3 { line[3..].trim() } else { "" };
        if path.is_empty() {
            Some(format!("status={}", &line[..2]))
        } else {
            Some(format!("status={} path={path}", &line[..2]))
        }
    }
    fn evidence(&self, command: &str, raw: &str) -> Vec<String> {
        let is_git_status = self.is_git_status(command);
        let is_git_data = self.is_git_data(command);
        let mut error_lines = Vec::new();
        let mut file_lines = Vec::new();
        let mut hunk_lines = Vec::new();
        let mut url_lines = Vec::new();
        for line in raw.lines() {
            let evidence_line = self.redact_for_summary(line);
            let status_evidence = if is_git_status {
                self.git_status_line_evidence(&evidence_line)
            } else {
                None
            };
            let blocker_line =
                is_git_status && self.is_long_git_status_blocker_line(&evidence_line);
            let error_match = if is_git_data {
                None
            } else {
                self.git_error.find(&evidence_line)
            };
            let file_match = self.file_line.find(&evidence_line);
            let hunk_match = self.diff_hunk.find(&evidence_line);
            let url_match = self.url.find(&evidence_line);
            let negative_match = if is_git_data {
                None
            } else {
                self.negative.find(&evidence_line)
            };
            if blocker_line {
                push(&mut error_lines, evidence_line.trim().to_string());
            } else if let Some(status) = status_evidence {
                push(&mut file_lines, status);
            } else if let (Some(f), Some(e)) = (file_match, error_match) {
                push(&mut error_lines, excerpt_multi(&evidence_line, &[f, e]));
            } else if let Some(e) = error_match {
                push(
                    &mut error_lines,
                    excerpt(&evidence_line, e.start(), e.end()),
                );
            } else if let Some(n) = negative_match {
                push(
                    &mut error_lines,
                    excerpt(&evidence_line, n.start(), n.end()),
                );
            } else if let Some(u) = url_match {
                push(&mut url_lines, excerpt(&evidence_line, u.start(), u.end()));
            } else if let Some(h) = hunk_match {
                push(&mut hunk_lines, excerpt(&evidence_line, h.start(), h.end()));
            } else if let Some(f) = file_match {
                push(&mut file_lines, excerpt(&evidence_line, f.start(), f.end()));
            }
        }
        error_lines
            .into_iter()
            .chain(url_lines)
            .chain(file_lines)
            .chain(hunk_lines)
            .take(12)
            .collect()
    }
    fn redact_for_summary(&self, text: &str) -> String {
        self.url
            .replace_all(text, |caps: &regex::Captures| {
                redact_url(caps.get(0).unwrap().as_str())
            })
            .to_string()
    }
}

/// Return the stable command-family label used by Tool Gateway execution and adapter reports.
///
/// The classifier is intentionally lightweight: cheap argv/text dispatch only. Unknown or
/// shell-wrapper commands fall back to `generic` rather than weakening evidence handling.
pub fn classify_command_family(command: &str) -> String {
    let cmd = command.trim();
    let words: Vec<&str> = cmd.split_whitespace().collect();
    if words.is_empty() {
        return "generic".into();
    }
    match words.as_slice() {
        ["sh", "-c", rest @ ..] | ["bash", "-lc", rest @ ..] | ["bash", "-c", rest @ ..] => {
            // Preserve real wrapper classification for analytics unless the wrapped command
            // starts with a P0 command. This keeps test/demo wrappers useful without claiming
            // universal shell interception.
            classify_wrapped_shell_command(&rest.join(" "))
        }
        _ => classify_direct_command(cmd),
    }
}

fn classify_wrapped_shell_command(command: &str) -> String {
    let first = command.split([';', '|']).next().unwrap_or(command).trim();
    let first = first
        .strip_prefix("exec ")
        .unwrap_or(first)
        .strip_prefix("env ")
        .unwrap_or(first)
        .trim();
    classify_direct_command(first)
}

fn classify_direct_command(cmd: &str) -> String {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    match words.as_slice() {
        ["git", "status", ..] => "git_status".into(),
        ["git", "diff", ..] => "git_diff".into(),
        ["git", "log", ..] => "git_log".into(),
        ["gh", "pr", "checks", ..] => "gh_pr_checks".into(),
        ["cargo", "test", ..] => "cargo_test".into(),
        ["cargo", "clippy", ..] => "cargo_clippy".into(),
        ["cargo", "build", ..] => "cargo_build".into(),
        ["cargo", "check", ..] => "cargo_check".into(),
        ["cargo", "fmt", rest @ ..] if args_contain_exact(rest, "--check") => {
            "cargo_fmt_check".into()
        }
        ["pytest", ..] | ["python", "-m", "pytest", ..] | ["python3", "-m", "pytest", ..] => {
            "pytest".into()
        }
        ["npm", "test", ..] => "npm_test".into(),
        ["pnpm", "test", ..] => "pnpm_test".into(),
        ["yarn", "test", ..] => "yarn_test".into(),
        ["npm", "install", ..] | ["npm", "ci", ..] => "npm_install".into(),
        ["pnpm", "install", ..] => "pnpm_install".into(),
        ["yarn", "install", ..] => "yarn_install".into(),
        ["npx", "vitest", ..] | ["vitest", ..] => "vitest".into(),
        ["next", "build", ..] | ["npx", "next", "build", ..] => "next".into(),
        ["eslint", ..] | ["npx", "eslint", ..] => "lint".into(),
        ["prettier", ..] | ["npx", "prettier", ..] => "prettier".into(),
        ["playwright", ..] | ["npx", "playwright", ..] => "playwright".into(),
        ["prisma", ..] | ["npx", "prisma", ..] => "prisma".into(),
        ["biome", ..] | ["npx", "biome", ..] => "biome".into(),
        ["turbo", ..] | ["npx", "turbo", ..] => "turbo".into(),
        ["nx", ..] | ["npx", "nx", ..] => "nx".into(),
        ["go", "test", ..] => "go_test".into(),
        ["golangci-lint", ..] => "golangci-lint".into(),
        ["df", ..] => "df".into(),
        ["du", ..] => "du".into(),
        ["find", ..] => "find".into(),
        ["grep", ..] | ["rg", ..] => "grep".into(),
        ["wc", ..] => "wc".into(),
        ["env", ..] | ["printenv", ..] => "env".into(),
        ["jq", ..] => "jq".into(),
        ["ps", ..] => "ps".into(),
        ["make", ..] => "make".into(),
        ["just", ..] => "just".into(),
        ["shellcheck", ..] => "shellcheck".into(),
        ["pre-commit", ..] => "pre-commit".into(),
        ["ruff", ..] => "ruff".into(),
        ["mypy", ..] => "mypy".into(),
        ["pip", "install", ..]
        | ["python", "-m", "pip", "install", ..]
        | ["python3", "-m", "pip", "install", ..] => "pip".into(),
        ["uv", "sync", ..] => "uv-sync".into(),
        ["poetry", "install", ..] => "poetry-install".into(),
        ["rspec", ..] => "rspec".into(),
        ["rubocop", ..] => "rubocop".into(),
        ["bundle", "install", ..] => "bundle-install".into(),
        ["dotnet", "build", ..] => "dotnet-build".into(),
        ["dotnet", "test", ..] => "dotnet".into(),
        ["terraform", "plan", ..] => "terraform-plan".into(),
        ["tofu", "plan", ..] => "tofu-plan".into(),
        ["helm", ..] => "helm".into(),
        ["kubectl", ..] => "kubectl".into(),
        ["docker", ..] => "docker".into(),
        ["aws", ..] => "aws".into(),
        ["gcloud", ..] => "gcloud".into(),
        ["systemctl", "status", ..] => "systemctl-status".into(),
        ["mvn", rest @ ..] | ["mvnw", rest @ ..] | ["./mvnw", rest @ ..]
            if java_test_args(rest) =>
        {
            "maven_test".into()
        }
        ["gradle", rest @ ..] | ["gradlew", rest @ ..] | ["./gradlew", rest @ ..]
            if java_test_args(rest) =>
        {
            "gradle_test".into()
        }
        ["tsc", rest @ ..]
        | ["npx", "tsc", rest @ ..]
        | ["pnpm", "exec", "tsc", rest @ ..]
        | ["yarn", "tsc", rest @ ..]
        | ["npm", "exec", "tsc", rest @ ..]
            if tsc_no_emit_args(rest) =>
        {
            "tsc_check".into()
        }
        _ => "generic".into(),
    }
}

fn args_contain_exact(args: &[&str], needle: &str) -> bool {
    args.contains(&needle)
}

fn tsc_no_emit_args(args: &[&str]) -> bool {
    args_contain_exact(args, "--noEmit") && !tsc_long_running_args(args)
}

fn java_test_args(args: &[&str]) -> bool {
    args.iter().any(|arg| {
        matches!(
            *arg,
            "test"
                | "verify"
                | "check"
                | "build"
                | ":test"
                | "testDebugUnitTest"
                | "connectedAndroidTest"
        ) || arg.ends_with(":test")
    })
}

fn tsc_long_running_args(args: &[&str]) -> bool {
    args.iter()
        .any(|arg| matches!(*arg, "--watch" | "-w" | "--watchFile" | "--watchDirectory"))
}

fn family_risk(
    family: &str,
    command: &str,
    raw: &str,
    exit_code: i32,
    is_git_github: bool,
    policy: &GitGithubToolPolicy,
) -> String {
    if exit_code != 0 {
        return "critical".into();
    }
    match family {
        "cargo_test" | "pytest" | "npm_test" | "pnpm_test" | "yarn_test" | "go_test"
        | "maven_test" | "gradle_test" => {
            if has_nonzero_test_failures(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() || has_zero_test_failures(raw) {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "gh_pr_checks" => {
            if has_nonzero_check_failures(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if Regex::new(r"(?i)(all checks passed|checks? passed|pass(?:ed|ing)?|success)")
                .expect("valid checks success regex")
                .is_match(raw)
            {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "cargo_clippy" | "cargo_build" | "cargo_check" => {
            if has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "cargo_fmt_check" => {
            if has_cargo_fmt_diff(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "tsc_check" => {
            if has_typescript_diagnostics(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        _ if built_in_filter_for_family(family)
            .map(|filter| filter.is_critical(raw))
            .unwrap_or(false) =>
        {
            "critical".into()
        }
        _ if built_in_filter_for_family(family).is_some()
            && (is_success(raw) || !raw.trim().is_empty()) =>
        {
            "success".into()
        }
        _ if is_git_github => policy.risk(command, raw, exit_code),
        _ if is_error(raw) => "critical".into(),
        _ if is_success(raw) || raw.trim().is_empty() => "success".into(),
        _ => "unknown".into(),
    }
}

fn has_cargo_fmt_diff(raw: &str) -> bool {
    raw.lines().any(|line| line.starts_with("Diff in "))
}

fn has_typescript_diagnostics(raw: &str) -> bool {
    Regex::new(r"(?m)\berror\s+TS[0-9]+:")
        .expect("valid TypeScript diagnostic regex")
        .is_match(raw)
}

fn has_zero_test_failures(raw: &str) -> bool {
    Regex::new(r"(?i)\b0\s+(?:failed|failures?|failing)\b")
        .expect("valid zero test failures regex")
        .is_match(raw)
}

fn has_nonzero_test_failures(raw: &str) -> bool {
    Regex::new(r"(?i)\b[1-9][0-9]*\s+(?:failed|failures?|failing)\b")
        .expect("valid nonzero test failures regex")
        .is_match(raw)
}

fn has_nonzero_check_failures(raw: &str) -> bool {
    let numeric_failure = Regex::new(
        r"(?i)\b[1-9][0-9]*\s+(?:failed|failing|cancel(?:led|ed)|timed? out|action_required)\b",
    )
    .expect("valid nonzero check failures regex");
    if numeric_failure.is_match(raw) {
        return true;
    }

    let zero_failure =
        Regex::new(r"(?i)\b0\s+(?:failed|failing)\b").expect("valid zero check failure regex");
    let failure_marker =
        Regex::new(r"(?i)\b(fail(?:ed|ing)?|error|cancel(?:led|ed)|timed? out|action_required)\b")
            .expect("valid check failure marker regex");
    raw.lines()
        .any(|line| failure_marker.is_match(line) && !zero_failure.is_match(line))
}

fn has_hard_failure_marker(raw: &str) -> bool {
    Regex::new(r"(?i)(panic|traceback|exception|fatal|denied|unauthori[sz]ed|forbidden|permission\s+denied|token\s+expired|assertion failed|not\s+successful|unsuccessful)")
        .expect("valid hard failure regex")
        .is_match(raw)
}

#[allow(dead_code)]
struct BuiltInFilterFixture {
    name: &'static str,
    raw: &'static str,
    exit_code: i32,
    expect_contains: &'static [&'static str],
}

struct BuiltInFilter {
    family: &'static str,
    strip_ansi: bool,
    replace: &'static [(&'static str, &'static str)],
    match_output: &'static [(&'static str, &'static [&'static str])],
    strip_lines_matching: &'static [&'static str],
    keep_lines_matching: &'static [&'static str],
    preserve_lines_matching: &'static [&'static str],
    unsafe_if_matches: &'static [&'static str],
    truncate_lines_at: usize,
    head_lines: usize,
    tail_lines: usize,
    max_lines: usize,
    on_empty: &'static str,
    human_auto_safe: bool,
    interactive_risk: &'static str,
    #[allow(dead_code)]
    fixtures: &'static [BuiltInFilterFixture],
}

const DEFAULT_DSL_FIXTURES: &[BuiltInFilterFixture] = &[BuiltInFilterFixture {
    name: "default-diagnostic",
    raw: "setup ok\nwarning: deprecated flag\nsrc/app.rs:42: error: example failure\ncompleted with diagnostics\n",
    exit_code: 0,
    expect_contains: &["warning", "src/app.rs:42", "raw_ref="],
}];

const DF_FIXTURES: &[BuiltInFilterFixture] = &[BuiltInFilterFixture {
    name: "df-high-usage",
    raw: "Filesystem      Size  Used Avail Use% Mounted on\n/dev/disk1s1    100G   95G    5G  95% /\n/dev/disk2s1    200G   20G  180G  10% /data\n",
    exit_code: 0,
    expect_contains: &["family=df", "/dev/disk1s1", "95%", "raw_ref="],
}];

macro_rules! tfy_filter {
    ($family:literal, human_safe = $human_safe:expr, risk = $risk:literal, fixtures = $fixtures:expr) => {
        BuiltInFilter {
            family: $family,
            strip_ansi: true,
            replace: &[],
            match_output: &[],
            strip_lines_matching: &[r"(?i)^(?:\s*\[?debug\]?|\s*trace:)"],
            keep_lines_matching: &[],
            preserve_lines_matching: &[
                r"(?i)(error|failed|failure|fatal|panic|warning|denied|unauthori[sz]ed|forbidden|deprecated)",
                r"(?i)(\b[A-Za-z0-9_./-]+\.(?:rs|ts|tsx|js|jsx|py|rb|go|java|kt|cs|json|ya?ml|toml|tf):[0-9]+)",
                r"(?i)(\b[8-9][0-9]%|100%)",
            ],
            unsafe_if_matches: &[
                r"(?i)(error|failed|failure|fatal|panic|denied|unauthori[sz]ed|forbidden|100%)",
            ],
            truncate_lines_at: 180,
            head_lines: 16,
            tail_lines: 8,
            max_lines: 32,
            on_empty: concat!($family, ": no relevant output"),
            human_auto_safe: $human_safe,
            interactive_risk: $risk,
            fixtures: $fixtures,
        }
    };
}

const BUILT_IN_FILTERS: &[BuiltInFilter] = &[
    tfy_filter!(
        "df",
        human_safe = true,
        risk = "none",
        fixtures = DF_FIXTURES
    ),
    tfy_filter!(
        "du",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "find",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "grep",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "wc",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "env",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "jq",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "ps",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "make",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "just",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "shellcheck",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pre-commit",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "npm_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pnpm_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "yarn_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "vitest",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "next",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "lint",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "prettier",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "playwright",
        human_safe = true,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "prisma",
        human_safe = true,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "biome",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "turbo",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "nx",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "ruff",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "mypy",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pip",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "uv-sync",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "poetry-install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "rspec",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "rubocop",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "bundle-install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "golangci-lint",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "dotnet-build",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "dotnet",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "terraform-plan",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "tofu-plan",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "helm",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "kubectl",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "docker",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "aws",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "gcloud",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "systemctl-status",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
];

fn built_in_filter_for_family(family: &str) -> Option<&'static BuiltInFilter> {
    BUILT_IN_FILTERS
        .iter()
        .find(|filter| filter.family == family)
}

fn built_in_filter_summary_candidate(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> Option<String> {
    let filter = built_in_filter_for_family(family)?;
    let filtered = apply_built_in_filter(filter, raw)?;
    let mut lines = vec![format!(
        "TFY command summary: {} family={} strategy=dsl exit={code} cmd={cmd}",
        risk.to_uppercase(),
        filter.family
    )];
    lines.push(format!(
        "- selected_lines={} original_lines={}",
        filtered.selected_lines,
        raw.lines().count()
    ));
    for line in filtered.preserved.iter().take(8) {
        lines.push(format!("- preserved: {line}"));
    }
    for line in filtered.lines.iter().take(filter.max_lines) {
        lines.push(format!("- {line}"));
    }
    if risk != "success" {
        lines.extend(evidence.iter().take(5).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    Some(lines.join("\n"))
}

struct FilteredOutput {
    lines: Vec<String>,
    preserved: Vec<String>,
    selected_lines: usize,
}

fn apply_built_in_filter(filter: &BuiltInFilter, raw: &str) -> Option<FilteredOutput> {
    let mut text = if filter.strip_ansi {
        strip_ansi_sequences(raw)
    } else {
        raw.to_string()
    };
    for (pattern, replacement) in filter.replace {
        let re = Regex::new(pattern).expect("valid built-in filter replace regex");
        text = re.replace_all(&text, *replacement).to_string();
    }
    for (pattern, unless_patterns) in filter.match_output {
        let re = Regex::new(pattern).expect("valid built-in filter match_output regex");
        let blocked = unless_patterns.iter().any(|unless| {
            Regex::new(unless)
                .expect("valid built-in filter unless regex")
                .is_match(&text)
        });
        if !re.is_match(&text) || blocked {
            return None;
        }
    }
    let strip_res = compile_regexes(filter.strip_lines_matching);
    let keep_res = compile_regexes(filter.keep_lines_matching);
    let preserve_res = compile_regexes(filter.preserve_lines_matching);
    let mut lines = Vec::new();
    let mut preserved = Vec::new();
    for line in text.lines() {
        let redacted = redact_public(line);
        let normalized = cap_to_chars(&norm(&redacted), filter.truncate_lines_at);
        if normalized.is_empty() || strip_res.iter().any(|re| re.is_match(&normalized)) {
            continue;
        }
        let is_preserved = preserve_res.iter().any(|re| re.is_match(&normalized));
        if is_preserved {
            push(&mut preserved, normalized.clone());
        }
        if !keep_res.is_empty()
            && !keep_res.iter().any(|re| re.is_match(&normalized))
            && !is_preserved
        {
            continue;
        }
        lines.push(normalized);
    }
    if lines.is_empty() && !filter.on_empty.is_empty() {
        lines.push(filter.on_empty.to_string());
    }
    let selected_lines = lines.len();
    lines = select_head_tail(
        lines,
        filter.head_lines,
        filter.tail_lines,
        filter.max_lines,
    );
    Some(FilteredOutput {
        lines,
        preserved,
        selected_lines,
    })
}

impl BuiltInFilter {
    fn is_critical(&self, raw: &str) -> bool {
        self.unsafe_if_matches.iter().any(|pattern| {
            Regex::new(pattern)
                .expect("valid built-in filter unsafe regex")
                .is_match(raw)
        })
    }
}

fn compile_regexes(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|pattern| Regex::new(pattern).expect("valid built-in filter regex"))
        .collect()
}

fn strip_ansi_sequences(text: &str) -> String {
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]")
        .expect("valid ansi regex")
        .replace_all(text, "")
        .to_string()
}

fn cap_to_chars(text: &str, limit: usize) -> String {
    if limit == 0 || text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push('…');
    out.push_str("[tfy: line capped]");
    out
}

fn select_head_tail(mut lines: Vec<String>, head: usize, tail: usize, max: usize) -> Vec<String> {
    if max == 0 || lines.len() <= max {
        return lines;
    }
    let head_count = head.min(max);
    let tail_count = tail.min(max.saturating_sub(head_count + 1));
    let mut selected = lines.drain(..head_count).collect::<Vec<_>>();
    selected.push("...".into());
    if tail_count > 0 {
        let tail_start = lines.len().saturating_sub(tail_count);
        selected.extend(lines.drain(tail_start..));
    }
    selected
}

#[cfg(test)]
pub(crate) fn validate_built_in_filter_fixtures() -> Result<()> {
    for filter in BUILT_IN_FILTERS {
        if filter.fixtures.is_empty() {
            bail!("built-in filter {} has no inline fixtures", filter.family);
        }
        for fixture in filter.fixtures {
            let raw_ref = format!("fixture_{}", fixture.name.replace('-', "_"));
            let risk = if fixture.exit_code != 0 || filter.is_critical(fixture.raw) {
                "critical"
            } else {
                "success"
            };
            let summary = built_in_filter_summary_candidate(
                filter.family,
                filter.family,
                fixture.exit_code,
                fixture.raw,
                &[],
                &raw_ref,
                risk,
            )
            .ok_or_else(|| anyhow::anyhow!("fixture {} produced no summary", fixture.name))?;
            for expected in fixture.expect_contains {
                if !summary.contains(expected) {
                    bail!(
                        "fixture {} for {} missing expected text {:?}: {}",
                        fixture.name,
                        filter.family,
                        expected,
                        summary
                    );
                }
            }
        }
    }
    Ok(())
}

fn rust_strategy_summary_candidate(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> Option<String> {
    match family {
        "git_status" => Some(git_status_summary(cmd, code, raw, evidence, rr, risk)),
        "git_diff" => Some(git_diff_summary(cmd, code, raw, evidence, rr, risk)),
        "git_log" => Some(git_log_summary(cmd, code, raw, evidence, rr, risk)),
        "gh_pr_checks" => Some(checks_summary(
            "gh_pr_checks",
            cmd,
            code,
            raw,
            evidence,
            rr,
            risk,
        )),
        "cargo_test" | "pytest" | "npm_test" | "pnpm_test" | "yarn_test" | "go_test"
        | "maven_test" | "gradle_test" => {
            Some(test_summary(family, cmd, code, raw, evidence, rr, risk))
        }
        "cargo_clippy" | "cargo_build" | "cargo_check" => {
            Some(build_summary(family, cmd, code, raw, evidence, rr, risk))
        }
        "cargo_fmt_check" => Some(cargo_fmt_summary(cmd, code, raw, evidence, rr, risk)),
        "tsc_check" => Some(tsc_summary(cmd, code, raw, evidence, rr, risk)),
        _ => None,
    }
}

fn git_status_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let changed = git_status_changed_count(raw);
    let branch = raw
        .lines()
        .find(|l| l.starts_with("On branch") || l.starts_with("##"))
        .map(redact_public);
    let state = if raw.trim().is_empty() || raw.contains("nothing to commit, working tree clean") {
        "clean"
    } else if risk == "critical" {
        "blocked"
    } else {
        "dirty"
    };
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_status exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- state={state} changed_paths={changed}"));
    if let Some(branch) = branch {
        lines.push(format!("- {branch}"));
    }
    lines.extend(evidence.iter().take(8).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn git_status_changed_count(raw: &str) -> String {
    let porcelain = raw
        .lines()
        .filter(|line| is_porcelain_status_prefix(line.trim_start()))
        .count();
    if porcelain > 0 {
        return porcelain.to_string();
    }
    let long_status = Regex::new(
        r"(?i)^\s*(?:modified|new file|deleted|renamed|copied|both modified|both added|both deleted|unmerged):\s+",
    )
    .expect("valid long git status regex");
    let long_count = raw
        .lines()
        .filter(|line| long_status.is_match(line))
        .count();
    if long_count > 0 {
        long_count.to_string()
    } else if raw.trim().is_empty() || raw.contains("nothing to commit, working tree clean") {
        "0".into()
    } else {
        "unknown".into()
    }
}

fn is_porcelain_status_prefix(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 3
        && bytes[2].is_ascii_whitespace()
        && matches!(
            (bytes[0], bytes[1]),
            (b'M', b' ')
                | (b' ', b'M')
                | (b'A', b' ')
                | (b' ', b'A')
                | (b'D', b' ')
                | (b' ', b'D')
                | (b'R', b' ')
                | (b' ', b'R')
                | (b'C', b' ')
                | (b' ', b'C')
                | (b'?', b'?')
                | (b'U', b'U')
                | (b'A', b'A')
                | (b'D', b'D')
                | (b'A', b'U')
                | (b'U', b'A')
                | (b'D', b'U')
                | (b'U', b'D')
        )
}

fn git_diff_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let patch_files = raw.lines().filter(|l| l.starts_with("diff --git ")).count();
    let stat_files = raw
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            let starts_with_digit = trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
            trimmed.contains('|') && !starts_with_digit
        })
        .count();
    let files = if patch_files > 0 {
        patch_files.to_string()
    } else if stat_files > 0 {
        stat_files.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let hunk_count = raw.lines().filter(|l| l.starts_with("@@ ")).count();
    let hunks = if hunk_count > 0 || raw.trim().is_empty() {
        hunk_count.to_string()
    } else {
        "unknown".into()
    };
    let patch_adds = raw
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++ "))
        .count();
    let patch_dels = raw
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("--- "))
        .count();
    let stat_insertions = stat_summary_count(raw, "insertion");
    let stat_deletions = stat_summary_count(raw, "deletion");
    let adds = if patch_files > 0 {
        patch_adds.to_string()
    } else if let Some(insertions) = stat_insertions {
        insertions.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let dels = if patch_files > 0 {
        patch_dels.to_string()
    } else if let Some(deletions) = stat_deletions {
        deletions.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_diff exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- files_changed={files} hunks={hunks} added_lines={adds} deleted_lines={dels}"
    ));
    lines.extend(evidence.iter().take(8).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn stat_summary_count(raw: &str, noun: &str) -> Option<usize> {
    let pattern = format!(r"(?i)\b([0-9]+)\s+{}s?\b", regex::escape(noun));
    let re = Regex::new(&pattern).expect("valid git stat count regex");
    let total = raw
        .lines()
        .filter_map(|line| {
            re.captures(line)
                .and_then(|caps| caps[1].parse::<usize>().ok())
        })
        .sum::<usize>();
    if total > 0 {
        Some(total)
    } else {
        None
    }
}

fn git_log_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let commit_count = raw
        .lines()
        .filter(|l| {
            l.starts_with("commit ")
                || Regex::new(r"^[0-9a-f]{7,}\b")
                    .expect("valid log regex")
                    .is_match(l.trim())
        })
        .count();
    let commits = if commit_count > 0 || raw.trim().is_empty() {
        commit_count.to_string()
    } else {
        "unknown".into()
    };
    let first = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| cap(&redact_public(l)))
        .unwrap_or_else(|| "no commits shown".into());
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_log exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- commits_shown={commits}"));
    lines.push(format!("- first={first}"));
    lines.extend(evidence.iter().take(5).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn checks_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let passed = Regex::new(r"(?i)\b(pass(?:ed|ing)?|success)\b")
        .expect("valid checks pass regex")
        .find_iter(raw)
        .count();
    let failed =
        Regex::new(r"(?i)\b(fail(?:ed|ing)?|error|cancel(?:led|ed)|timed? out|action_required)\b")
            .expect("valid checks fail regex")
            .find_iter(raw)
            .count();
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- checks_pass_like={passed} checks_fail_like={failed}"
    ));
    lines.extend(evidence.iter().take(10).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn test_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let tests = Regex::new(r"(?i)(\d+)\s+(?:tests?|passed)")
        .expect("valid test count regex")
        .captures_iter(raw)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
        .max()
        .or_else(|| {
            Regex::new(r"(?i)Tests run:\s*([0-9]+)")
                .expect("valid maven test count regex")
                .captures_iter(raw)
                .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
                .max()
        })
        .unwrap_or(0);
    let failures = Regex::new(r"(?i)(\d+)\s+(?:failed|failures?)")
        .expect("valid failure count regex")
        .captures_iter(raw)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
        .max()
        .or_else(|| {
            Regex::new(r"(?i)(?:Failures|Errors):\s*([1-9][0-9]*)")
                .expect("valid maven failure count regex")
                .captures_iter(raw)
                .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
                .max()
        })
        .unwrap_or_else(|| if risk == "critical" { 1 } else { 0 });
    let failed_names = collect_lines(
        raw,
        r"(?i)(FAILED|FAILURE!|<<< FAILURE!|failures:|Tests run:|There are test failures|---- .+ stdout|panic|assert|expected|got|left:|right:|Traceback|Error:|AssertionError|org\.|\.java:[0-9]+)",
        10,
    );
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- tests_observed={tests} failures_observed={failures}"
    ));
    lines.extend(failed_names.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn build_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let warnings = Regex::new(r"(?i)\bwarning[: ]")
        .expect("valid warning regex")
        .find_iter(raw)
        .count();
    let errors = Regex::new(r"(?i)\berror(?:\[|:|s?\b)")
        .expect("valid error regex")
        .find_iter(raw)
        .count();
    let finished = raw
        .lines()
        .find(|l| l.contains("Finished ") || l.contains("Checking ") || l.contains("Compiling "))
        .map(|l| cap(&redact_public(l)))
        .unwrap_or_else(|| "build output summarized".into());
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- warnings={warnings} errors={errors}"));
    lines.push(format!("- {finished}"));
    lines.extend(evidence.iter().take(10).map(|e| format!("- evidence: {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn cargo_fmt_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let diff_count = raw
        .lines()
        .filter(|line| line.starts_with("Diff in "))
        .count();
    let diffs = if diff_count > 0 || raw.trim().is_empty() {
        diff_count.to_string()
    } else {
        "unknown".into()
    };
    let diff_lines = collect_lines(raw, r"(?i)(^Diff in |rustfmt|formatted|formatting)", 10);
    let mut lines = vec![format!(
        "TFY command summary: {} family=cargo_fmt_check exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- format_diffs={diffs}"));
    lines.extend(diff_lines.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn tsc_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let diagnostics_count = Regex::new(r"(?m)\berror\s+TS[0-9]+:")
        .expect("valid TypeScript diagnostic count regex")
        .find_iter(raw)
        .count();
    let diagnostics = if diagnostics_count > 0 || raw.trim().is_empty() {
        diagnostics_count.to_string()
    } else {
        "unknown".into()
    };
    let type_errors = collect_lines(raw, r"(?i)(error TS[0-9]+:|Found [0-9]+ errors?)", 10);
    let mut lines = vec![format!(
        "TFY command summary: {} family=tsc_check exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- diagnostics={diagnostics}"));
    lines.extend(type_errors.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn collect_lines(raw: &str, pattern: &str, limit: usize) -> Vec<String> {
    let re = Regex::new(pattern).expect("valid collect regex");
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = redact_public(line);
        if re.is_match(&line) {
            push(&mut out, cap(&norm(&line)));
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn redact_public(text: &str) -> String {
    redact_secret_like(&GitGithubToolPolicy::new().redact_for_summary(text))
}

fn redact_secret_like(text: &str) -> String {
    let assignment = Regex::new(
        r"(?i)\b([A-Z0-9_.-]*(?:api[_-]?key|access[_-]?token|auth[_-]?token|token|secret|password))\s*([:=])\s*[A-Za-z0-9._~+/=-]{6,}",
    )
    .expect("valid secret assignment regex");
    let bearer =
        Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]{8,}").expect("valid bearer secret regex");
    let text = assignment.replace_all(text, "$1$2[REDACTED]");
    bearer.replace_all(&text, "$1 [REDACTED]").to_string()
}

fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let stop = rest.find(['?', '#']).unwrap_or(rest.len());
    let no_query = &rest[..stop];
    let (authority, path) = match no_query.find('/') {
        Some(idx) => (&no_query[..idx], &no_query[idx..]),
        None => (no_query, ""),
    };
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = match host_port.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            format!("{host}:{port}")
        }
        Some((host, _)) => host.to_string(),
        None => host_port.to_string(),
    };
    format!("{scheme}://{host}{path}")
}

fn is_error(raw: &str) -> bool {
    Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL|not\s+successful|unsuccessful|failing|cancel(?:led|ed)|timed?\s+out|action_required|expired|unauthori[sz]ed|forbidden|token\s+expired|authentication|permission\s+denied)")
        .expect("valid error regex")
        .is_match(raw)
}
fn is_success(raw: &str) -> bool {
    let success = Regex::new(
        r"(?i)(\bpassed\b|\bok\b|\bdone\b|\bclean\b|up[ -]?to[ -]?date|nothing to commit|no changes|0 failures?|all checks passed|checks? passed|successful)",
    )
    .expect("valid success regex");
    success.is_match(raw)
}
fn evidence(raw: &str) -> Vec<String> {
    let err = Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL|not\s+successful|unsuccessful|failing|cancel(?:led|ed)|timed?\s+out|action_required|expired|unauthori[sz]ed|forbidden|token\s+expired|authentication|permission\s+denied)").expect("valid error regex");
    let file = Regex::new(r"[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md)(?::\d+)?")
        .expect("valid file reference regex");
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for line in raw.lines() {
        let line = redact_public(line);
        let em = err.find(&line);
        let fm = file.find(&line);
        match (em, fm) {
            (Some(e), Some(f)) => push(&mut errors, excerpt_multi(&line, &[f, e])),
            (Some(e), None) => push(&mut errors, excerpt(&line, e.start(), e.end())),
            (None, Some(f)) => push(&mut files, excerpt(&line, f.start(), f.end())),
            _ => {}
        }
    }
    errors.into_iter().chain(files).take(12).collect()
}
fn push(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && !v.contains(&s) {
        v.push(s)
    }
}
fn excerpt_multi(line: &str, ms: &[regex::Match]) -> String {
    let Some(start) = ms.iter().map(|m| m.start()).min() else {
        return String::new();
    };
    let Some(end) = ms.iter().map(|m| m.end()).max() else {
        return String::new();
    };
    if end - start > 240 {
        ms.iter()
            .map(|m| excerpt(line, m.start(), m.end()))
            .collect::<Vec<_>>()
            .join(" … ")
    } else {
        excerpt(line, start, end)
    }
}
fn excerpt(line: &str, start: usize, end: usize) -> String {
    let limit = 240;
    if line.len() <= limit {
        return norm(line);
    }
    let ctx = ((limit - (end - start).min(limit)) / 2).max(20);
    let s = floor_char_boundary(line, start.saturating_sub(ctx));
    let e = ceil_char_boundary(line, (end + ctx).min(line.len()));
    let mut out = String::new();
    if s > 0 {
        out.push('…')
    }
    out.push_str(&line[s..e]);
    if e < line.len() {
        out.push('…')
    }
    norm(&out)
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

fn norm(s: &str) -> String {
    Regex::new(r"\s+")
        .expect("valid whitespace regex")
        .replace_all(s, " ")
        .trim()
        .to_string()
}
fn cap(s: &str) -> String {
    const LIMIT: usize = 240;
    if s.len() <= LIMIT {
        return s.into();
    }
    let end = s
        .char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx <= LIMIT)
        .last()
        .unwrap_or(0);
    format!("{}…[tfy: line capped]", &s[..end])
}

fn critical(cmd: &str, code: i32, e: &[String], rr: &str) -> String {
    let body = if e.is_empty() {
        "- critical output present; request raw for details".into()
    } else {
        e.iter()
            .map(|x| format!("- {x}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("TFY command summary: CRITICAL exit={code} cmd={cmd}\n{body}\nraw_ref={rr}\n")
}

fn success(cmd: &str, code: i32, raw: &str, rr: &str) -> String {
    let first = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no output");
    let first = redact_public(first);
    format!(
        "TFY command summary: SUCCESS exit={code} cmd={cmd}; {}; raw_ref={rr}\n",
        cap(&first)
    )
}

fn unknown(cmd: &str, code: i32, raw: &str, e: &[String], rr: &str) -> String {
    let lines: Vec<_> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let normalized = norm(l);
            cap(&redact_public(&normalized))
        })
        .collect();
    let mut selected = lines.iter().take(5).cloned().collect::<Vec<_>>();
    if lines.len() > 8 {
        selected.push("...".into());
        selected.extend(
            lines
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        );
    }
    selected.extend(e.iter().take(5).map(|x| format!("evidence: {x}")));
    let body = if selected.is_empty() {
        "no output".into()
    } else {
        selected.join("\n")
    };
    format!("TFY command summary: UNKNOWN exit={code} cmd={cmd}\n{body}\nraw_ref={rr}\n")
}

fn cap_summary_preserving_raw_ref(
    summary: String,
    raw_ref: &str,
    max_summary_bytes: Option<usize>,
) -> String {
    let Some(limit) = max_summary_bytes else {
        return summary;
    };
    if summary.len() <= limit {
        return summary;
    }
    let suffix = format!("\n…\nraw_ref={raw_ref}\n");
    let prefix_limit = limit.saturating_sub(suffix.len());
    let end = floor_char_boundary(&summary, prefix_limit);
    format!("{}{}", &summary[..end], suffix)
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
