use anyhow::{bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
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

#[derive(Debug, Clone)]
pub struct RawStore {
    root: PathBuf,
}

impl RawStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
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
            match OpenOptions::new().create_new(true).write(true).open(&path) {
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
            let _summary_limit = max_summary_bytes;
            compress(
                &store,
                &command.join(" "),
                &raw,
                out.status.code().unwrap_or(-1),
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

fn compress(store: &RawStore, command: &str, raw: &str, exit_code: i32) -> Result<CommandSummary> {
    let raw_ref = store.put(command, raw, exit_code)?;
    let evidence = evidence(raw);
    let risk = if exit_code != 0 || is_error(raw) {
        "critical"
    } else if is_success(raw) || raw.trim().is_empty() {
        "success"
    } else {
        "unknown"
    };
    let summary = match risk {
        "critical" => critical(command, exit_code, &evidence, &raw_ref),
        "success" => success(command, exit_code, raw, &raw_ref),
        _ => unknown(command, exit_code, raw, &evidence, &raw_ref),
    };
    let savings_pct = if raw.is_empty() {
        0.0
    } else {
        ((raw.len() as f64 - summary.len() as f64) / raw.len() as f64 * 10000.0).round() / 100.0
    };
    Ok(CommandSummary {
        command: command.into(),
        exit_code,
        risk: risk.into(),
        summary_chars: summary.len(),
        raw_chars: raw.len(),
        savings_pct,
        summary,
        raw_ref,
        evidence,
    })
}

fn is_error(raw: &str) -> bool {
    Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|FAIL)").expect("valid error regex").is_match(raw)
}
fn is_success(raw: &str) -> bool {
    Regex::new(
        r"(?i)(success|passed|ok|done|clean|up.to.date|nothing to commit|no changes|0 failures?)",
    )
    .expect("valid success regex")
    .is_match(raw)
}
fn evidence(raw: &str) -> Vec<String> {
    let err = Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|FAIL)").expect("valid error regex");
    let file = Regex::new(r"[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md)(?::\d+)?")
        .expect("valid file reference regex");
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for line in raw.lines() {
        let em = err.find(line);
        let fm = file.find(line);
        match (em, fm) {
            (Some(e), Some(f)) => push(&mut errors, excerpt_multi(line, &[f, e])),
            (Some(e), None) => push(&mut errors, excerpt(line, e.start(), e.end())),
            (None, Some(f)) => push(&mut files, excerpt(line, f.start(), f.end())),
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
    let s = start.saturating_sub(ctx);
    let e = (end + ctx).min(line.len());
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
fn norm(s: &str) -> String {
    Regex::new(r"\s+")
        .expect("valid whitespace regex")
        .replace_all(s, " ")
        .trim()
        .to_string()
}
fn cap(s: &str) -> String {
    if s.len() <= 240 {
        s.into()
    } else {
        format!("{}…[tfy: line capped]", &s[..240])
    }
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
    format!(
        "TFY command summary: SUCCESS exit={code} cmd={cmd}; {}; raw_ref={rr}\n",
        cap(first)
    )
}
fn unknown(cmd: &str, code: i32, raw: &str, e: &[String], rr: &str) -> String {
    let lines: Vec<_> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| cap(&norm(l)))
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
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
