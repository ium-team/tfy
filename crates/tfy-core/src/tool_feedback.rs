use anyhow::{bail, Result};
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
    pub raw_ref: String,
    pub raw_chars: usize,
    pub summary_chars: usize,
    pub savings_pct: f64,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    Auto,
    Generic,
    GitGithub,
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
            let payload = serde_json::json!({"command":command,"exit_code":exit_code,"raw":raw,"created_ns":nonce});
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
        let path = self.path_for(raw_ref)?;
        let text = fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;
        let raw = v["raw"].as_str().unwrap_or("");
        if let Some(needle) = around {
            let lines: Vec<_> = raw.lines().collect();
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
                return Ok(out.join("\n") + "\n");
            }
        }
        Ok(raw.to_string())
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
            let mut raw = String::from_utf8_lossy(&out.stdout).to_string();
            raw.push_str(&String::from_utf8_lossy(&out.stderr));
            compress(
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
    let raw_ref = store.put(command, raw, exit_code)?;
    let policy = GitGithubToolPolicy::new();
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
    let risk = if is_git_github {
        policy.risk(command, raw, exit_code)
    } else if exit_code != 0 || is_error(raw) {
        "critical".to_string()
    } else if is_success(raw) || raw.trim().is_empty() {
        "success".to_string()
    } else {
        "unknown".to_string()
    };
    let display_command = redact_public(command);
    let summary = match risk.as_str() {
        "critical" => critical(&display_command, exit_code, &evidence, &raw_ref),
        "success" => success(&display_command, exit_code, raw, &raw_ref),
        _ => unknown(&display_command, exit_code, raw, &evidence, &raw_ref),
    };
    let summary = cap_summary_preserving_raw_ref(summary, &raw_ref, max_summary_bytes);
    let savings_pct = if raw.is_empty() {
        0.0
    } else {
        ((raw.len() as f64 - summary.len() as f64) / raw.len() as f64 * 10000.0).round() / 100.0
    };
    Ok(CommandSummary {
        command: display_command,
        exit_code,
        risk,
        summary_chars: summary.len(),
        raw_chars: raw.len(),
        savings_pct,
        summary,
        raw_ref,
        evidence,
    })
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

fn redact_public(text: &str) -> String {
    GitGithubToolPolicy::new().redact_for_summary(text)
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
