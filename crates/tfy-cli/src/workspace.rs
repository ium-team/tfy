use crate::util::{print_json, read_payload};
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tfy_core::{restore_file_payload, RestorePayload};
use tfy_runtime::{Origin, OriginKind, ValidationStatus};

#[derive(Subcommand)]
pub(crate) enum WorkspaceCmd {
    /// Validate a WorkspaceApplyPlan and print a no-mutation preview.
    Validate(WorkspaceValidateCmd),
    /// Apply a previously validated WorkspaceApplyPlan using plan hash + per-op proofs.
    Apply(WorkspaceApplyCmd),
    /// Split a larger refactor plan into deterministic verification chunks without mutating.
    RefactorPlan(WorkspaceRefactorCmd),
}

#[derive(Args, Clone)]
pub(crate) struct WorkspaceValidateCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct WorkspaceApplyCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    #[arg(long = "plan-hash")]
    pub plan_hash: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct WorkspaceRefactorCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    #[arg(long, default_value_t = 5)]
    pub chunk_size: usize,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceApplyPlan {
    pub plan_id: String,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub operations: Vec<WorkspaceOperation>,
    #[serde(default)]
    pub policy: WorkspacePolicy,
    #[serde(default)]
    pub validation_status: Option<ValidationStatus>,
    #[serde(default)]
    pub validation_proof: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspacePolicy {
    #[serde(default)]
    pub allow_fuzzy_apply: bool,
    #[serde(default = "default_fuzzy_confidence")]
    pub fuzzy_confidence_threshold: f64,
    #[serde(default = "default_rollback")]
    pub rollback_strategy: String,
}

impl Default for WorkspacePolicy {
    fn default() -> Self {
        Self {
            allow_fuzzy_apply: false,
            fuzzy_confidence_threshold: default_fuzzy_confidence(),
            rollback_strategy: default_rollback(),
        }
    }
}

fn default_rollback() -> String {
    "journal".into()
}

fn default_fuzzy_confidence() -> f64 {
    0.95
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceOperation {
    pub op_id: String,
    pub kind: WorkspaceOpKind,
    #[serde(default)]
    pub path_before: Option<PathBuf>,
    #[serde(default)]
    pub path_after: Option<PathBuf>,
    #[serde(default)]
    pub base_file_hash: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub restore_payload: Option<RestorePayload>,
    #[serde(default)]
    pub anchor_before: Option<String>,
    #[serde(default)]
    pub replacement: Option<String>,
    #[serde(default = "default_expected_occurrences")]
    pub expected_occurrences: usize,
    #[serde(default)]
    pub per_op_proof: Option<OperationProof>,
    #[serde(default)]
    pub restored_preview_hash: Option<String>,
    #[serde(default)]
    pub conflict_policy: Option<String>,
    #[serde(default)]
    pub selected_range: Option<SelectedRange>,
    #[serde(default)]
    pub full_file_scope: bool,
    #[serde(default)]
    pub confidence: Option<f64>,
}

fn default_expected_occurrences() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OperationProof {
    pub proof_id: String,
    pub source_ref: String,
    pub validation_status: ValidationStatus,
    pub authority: String,
    #[serde(default)]
    pub base_file_hash: Option<String>,
    #[serde(default)]
    pub restored_preview_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SelectedRange {
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceOpKind {
    ModifyExact,
    ModifyFuzzy,
    Add,
    Delete,
    Rename,
    Move,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorkspaceValidationReport {
    pub status: String,
    pub plan_id: String,
    pub plan_hash: String,
    pub operation_count: usize,
    pub operations: Vec<OperationPreview>,
    pub preview_diff: String,
    pub validation_proof: String,
    pub rollback_strategy: String,
    pub mutated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OperationPreview {
    pub op_id: String,
    pub kind: String,
    pub path_before: Option<String>,
    pub path_after: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorkspaceApplyReport {
    pub status: String,
    pub plan_id: String,
    pub plan_hash: String,
    pub applied_operations: usize,
    pub rollback_journal: Vec<RollbackEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RefactorPlanReport {
    pub status: String,
    pub plan_id: String,
    pub chunk_size: usize,
    pub chunk_count: usize,
    pub chunks: Vec<RefactorChunk>,
    pub verification: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RefactorChunk {
    pub chunk_id: String,
    pub operation_ids: Vec<String>,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RollbackEntry {
    pub path: String,
    pub existed_before: bool,
    pub content_sha256: Option<String>,
}

pub(crate) fn execute_workspace(cmd: WorkspaceCmd) -> Result<()> {
    match cmd {
        WorkspaceCmd::Validate(cmd) => {
            let plan = read_plan(cmd.payload)?;
            let report = validate_plan(&plan)?;
            if cmd.json {
                print_json(&report)?;
            } else {
                print!("{}", report.preview_diff);
            }
        }
        WorkspaceCmd::Apply(cmd) => {
            let plan = read_plan(cmd.payload)?;
            let report = apply_plan(&plan, &cmd.plan_hash)?;
            if cmd.json {
                print_json(&report)?;
            } else {
                println!(
                    "applied {} operation(s) plan_hash={}",
                    report.applied_operations, report.plan_hash
                );
            }
        }
        WorkspaceCmd::RefactorPlan(cmd) => {
            let plan = read_plan(cmd.payload)?;
            let report = refactor_plan(&plan, cmd.chunk_size)?;
            if cmd.json {
                print_json(&report)?;
            } else {
                println!(
                    "refactor plan {}: {} chunk(s)",
                    report.plan_id, report.chunk_count
                );
                for chunk in report.chunks {
                    println!("{} {}", chunk.chunk_id, chunk.operation_ids.join(","));
                }
            }
        }
    }
    Ok(())
}

fn read_plan(payload: Option<PathBuf>) -> Result<WorkspaceApplyPlan> {
    Ok(serde_json::from_str(&read_payload(payload)?)?)
}

fn canonical_plan_value(plan: &WorkspaceApplyPlan) -> serde_json::Value {
    serde_json::json!({
        "plan_id": plan.plan_id,
        "origin": plan.origin,
        "policy": plan.policy,
        "operations": plan.operations,
    })
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn plan_hash(plan: &WorkspaceApplyPlan) -> Result<String> {
    Ok(format!(
        "tfy_{}",
        &sha256_hex(&serde_json::to_string(&canonical_plan_value(plan))?)[..16]
    ))
}

fn file_hash(path: &Path) -> Result<String> {
    Ok(sha256_hex(
        &fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
    ))
}

fn validate_safe_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!(
            "workspace operation path must be relative and stay inside workspace: {}",
            path.display()
        );
    }
    let root = std::env::current_dir()?.canonicalize()?;
    let mut cursor = root.clone();
    for component in path.components() {
        if let Component::Normal(part) = component {
            cursor.push(part);
            if let Ok(meta) = fs::symlink_metadata(&cursor) {
                if meta.file_type().is_symlink() {
                    bail!(
                        "workspace operation path must not traverse symlink: {}",
                        path.display()
                    );
                }
            }
        }
    }
    let existing = root.join(path);
    let canonical_target = if existing.exists() {
        Some(existing.canonicalize()?)
    } else if let Some(parent) = existing.parent().filter(|p| p.exists()) {
        Some(parent.canonicalize()?)
    } else {
        None
    };
    if let Some(canonical_target) = canonical_target {
        if !canonical_target.starts_with(&root) {
            bail!(
                "workspace operation path escapes workspace root: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn validate_origin(origin: &Origin) -> Result<()> {
    if matches!(origin.kind, OriginKind::HumanCli)
        || !origin.intercepted
        || origin.user_shell_mutated
    {
        bail!(
            "workspace apply plan requires explicit agent/test origin without human shell mutation"
        );
    }
    Ok(())
}

fn require_proof(op: &WorkspaceOperation) -> Result<()> {
    let proof = op
        .per_op_proof
        .as_ref()
        .ok_or_else(|| anyhow!("operation {} missing per_op_proof", op.op_id))?;
    if proof.proof_id.trim().is_empty() || proof.source_ref.trim().is_empty() {
        bail!(
            "operation {} per_op_proof requires proof_id and source_ref",
            op.op_id
        );
    }
    if proof.validation_status != ValidationStatus::Valid {
        bail!("operation {} per_op_proof is not valid", op.op_id);
    }
    if proof.authority != "workspace_apply" {
        bail!(
            "operation {} per_op_proof authority must be workspace_apply",
            op.op_id
        );
    }
    if let Some(expected) = op.base_file_hash.as_deref() {
        if proof.base_file_hash.as_deref() != Some(expected) {
            bail!("operation {} per_op_proof base hash mismatch", op.op_id);
        }
    }
    if let Some(expected) = op.restored_preview_hash.as_deref() {
        if proof.restored_preview_hash.as_deref() != Some(expected) {
            bail!("operation {} per_op_proof preview hash mismatch", op.op_id);
        }
    }
    Ok(())
}

fn validate_scope_authority(op: &WorkspaceOperation) -> Result<()> {
    if !op.full_file_scope && op.selected_range.is_none() {
        bail!(
            "operation {} requires selected_range or full_file_scope authority",
            op.op_id
        );
    }
    if let Some(range) = &op.selected_range {
        if range.byte_start >= range.byte_end {
            bail!("operation {} selected_range is empty or reversed", op.op_id);
        }
    }
    Ok(())
}

fn operation_content(op: &WorkspaceOperation) -> Result<Option<String>> {
    if let Some(payload) = &op.restore_payload {
        return Ok(Some(restore_file_payload(payload.clone())?.file_code));
    }
    Ok(op.content.clone())
}

fn validate_conflicts(plan: &WorkspaceApplyPlan) -> Result<()> {
    for (idx, left) in plan.operations.iter().enumerate() {
        for right in plan.operations.iter().skip(idx + 1) {
            if let Some(path_before) = left
                .path_before
                .as_ref()
                .filter(|_| left.path_before == right.path_before)
            {
                bail!(
                    "workspace conflict: operations {} and {} both mutate {}; merge same-file mutations into one full-file candidate",
                    left.op_id,
                    right.op_id,
                    path_before.display()
                );
            }
        }
    }
    Ok(())
}

fn validate_fuzzy_candidate(
    plan: &WorkspaceApplyPlan,
    op: &WorkspaceOperation,
    path: &Path,
) -> Result<String> {
    if !plan.policy.allow_fuzzy_apply {
        bail!("modify_fuzzy requires policy.allow_fuzzy_apply=true");
    }
    let confidence = op.confidence.unwrap_or(0.0);
    if confidence < plan.policy.fuzzy_confidence_threshold {
        bail!(
            "modify_fuzzy {} confidence {confidence:.2} below threshold {:.2}",
            op.op_id,
            plan.policy.fuzzy_confidence_threshold
        );
    }
    let anchor = op
        .anchor_before
        .as_deref()
        .ok_or_else(|| anyhow!("modify_fuzzy {} requires anchor_before", op.op_id))?;
    let restored_replacement = operation_content(op)?;
    let replacement = restored_replacement
        .as_deref()
        .or(op.replacement.as_deref())
        .or(op.content.as_deref())
        .ok_or_else(|| {
            anyhow!(
                "modify_fuzzy {} requires restore_payload/replacement/content",
                op.op_id
            )
        })?;
    let text = fs::read_to_string(path)?;
    let expected_base = op
        .base_file_hash
        .as_deref()
        .ok_or_else(|| anyhow!("modify_fuzzy {} requires base_file_hash", op.op_id))?;
    let actual_base = sha256_hex(&text);
    if actual_base != expected_base {
        bail!("modify_fuzzy {} stale base hash", op.op_id);
    }
    let occurrences = text.match_indices(anchor).count();
    if occurrences != op.expected_occurrences || occurrences != 1 {
        bail!(
            "modify_fuzzy {} ambiguous anchor occurrences={occurrences}",
            op.op_id
        );
    }
    let candidate = text.replacen(anchor, replacement, 1);
    let expected_preview = op
        .restored_preview_hash
        .as_deref()
        .ok_or_else(|| anyhow!("modify_fuzzy {} requires restored_preview_hash", op.op_id))?;
    if sha256_hex(&candidate) != expected_preview {
        bail!("modify_fuzzy {} restored preview hash mismatch", op.op_id);
    }
    Ok(candidate)
}

pub(crate) fn validate_plan(plan: &WorkspaceApplyPlan) -> Result<WorkspaceValidationReport> {
    if plan.plan_id.trim().is_empty() {
        bail!("workspace plan requires plan_id");
    }
    if plan.operations.is_empty() {
        bail!("workspace plan requires at least one operation");
    }
    validate_origin(&plan.origin)?;
    validate_conflicts(plan)?;
    let mut previews = Vec::new();
    let mut diff = String::new();
    let mut destinations = BTreeMap::<PathBuf, String>::new();
    for op in &plan.operations {
        require_proof(op)?;
        validate_scope_authority(op)?;
        let before = op.path_before.as_deref();
        let after = op.path_after.as_deref();
        if let Some(path) = before {
            validate_safe_path(path)?;
        }
        if let Some(path) = after {
            validate_safe_path(path)?;
        }
        match op.kind {
            WorkspaceOpKind::ModifyExact => {
                let path = before
                    .ok_or_else(|| anyhow!("modify_exact {} requires path_before", op.op_id))?;
                let content = operation_content(op)?.ok_or_else(|| {
                    anyhow!(
                        "modify_exact {} requires content or restore_payload",
                        op.op_id
                    )
                })?;
                if let Some(expected_preview) = op.restored_preview_hash.as_deref() {
                    if sha256_hex(&content) != expected_preview {
                        bail!("modify_exact {} restored preview hash mismatch", op.op_id);
                    }
                }
                let expected = op
                    .base_file_hash
                    .as_deref()
                    .ok_or_else(|| anyhow!("modify_exact {} requires base_file_hash", op.op_id))?;
                let actual = file_hash(path)?;
                if actual != expected {
                    bail!("modify_exact {} stale base hash", op.op_id);
                }
                if content.is_empty() {
                    bail!("modify_exact {} empty content is not delete", op.op_id);
                }
                diff.push_str(&format!(
                    "--- {}\n+++ {}\n@@ replace file\n{}\n",
                    path.display(),
                    path.display(),
                    content
                ));
                previews.push(preview(op, "exact file modification validated"));
            }
            WorkspaceOpKind::ModifyFuzzy => {
                let path = before
                    .ok_or_else(|| anyhow!("modify_fuzzy {} requires path_before", op.op_id))?;
                if !path.exists() {
                    bail!("modify_fuzzy {} target missing", op.op_id);
                }
                let candidate = validate_fuzzy_candidate(plan, op, path)?;
                diff.push_str(&format!(
                    "--- {}\n+++ {}\n@@ fuzzy unique-anchor replace\n{}\n",
                    path.display(),
                    path.display(),
                    candidate
                ));
                previews.push(preview(op, "fuzzy unique-anchor modification validated"));
            }
            WorkspaceOpKind::Add => {
                let path = after.ok_or_else(|| anyhow!("add {} requires path_after", op.op_id))?;
                if path.exists() {
                    bail!("add {} destination already exists", op.op_id);
                }
                let add_content = operation_content(op)?.unwrap_or_default();
                if let Some(expected_preview) = op.restored_preview_hash.as_deref() {
                    if sha256_hex(&add_content) != expected_preview {
                        bail!("add {} restored preview hash mismatch", op.op_id);
                    }
                }
                if destinations
                    .insert(path.to_path_buf(), op.op_id.clone())
                    .is_some()
                {
                    bail!("duplicate destination {}", path.display());
                }
                diff.push_str(&format!(
                    "--- /dev/null\n+++ {}\n{}\n",
                    path.display(),
                    add_content
                ));
                previews.push(preview(op, "add validated"));
            }
            WorkspaceOpKind::Delete => {
                let path =
                    before.ok_or_else(|| anyhow!("delete {} requires path_before", op.op_id))?;
                let expected = op
                    .base_file_hash
                    .as_deref()
                    .ok_or_else(|| anyhow!("delete {} requires base_file_hash", op.op_id))?;
                if file_hash(path)? != expected {
                    bail!("delete {} stale base hash", op.op_id);
                }
                diff.push_str(&format!(
                    "--- {}\n+++ /dev/null\n@@ delete file\n",
                    path.display()
                ));
                previews.push(preview(op, "delete validated"));
            }
            WorkspaceOpKind::Rename | WorkspaceOpKind::Move => {
                let src = before
                    .ok_or_else(|| anyhow!("rename/move {} requires path_before", op.op_id))?;
                let dst =
                    after.ok_or_else(|| anyhow!("rename/move {} requires path_after", op.op_id))?;
                let expected = op
                    .base_file_hash
                    .as_deref()
                    .ok_or_else(|| anyhow!("rename/move {} requires base_file_hash", op.op_id))?;
                if file_hash(src)? != expected {
                    bail!("rename/move {} stale base hash", op.op_id);
                }
                if dst.exists() {
                    bail!("rename/move {} destination exists", op.op_id);
                }
                if destinations
                    .insert(dst.to_path_buf(), op.op_id.clone())
                    .is_some()
                {
                    bail!("duplicate destination {}", dst.display());
                }
                diff.push_str(&format!("rename {} -> {}\n", src.display(), dst.display()));
                previews.push(preview(op, "rename/move validated"));
            }
        }
    }
    let hash = plan_hash(plan)?;
    Ok(WorkspaceValidationReport {
        status: "valid".into(),
        plan_id: plan.plan_id.clone(),
        plan_hash: hash.clone(),
        operation_count: plan.operations.len(),
        operations: previews,
        preview_diff: diff,
        validation_proof: hash,
        rollback_strategy: plan.policy.rollback_strategy.clone(),
        mutated: false,
    })
}

fn preview(op: &WorkspaceOperation, message: &str) -> OperationPreview {
    OperationPreview {
        op_id: op.op_id.clone(),
        kind: format!("{:?}", op.kind).to_ascii_lowercase(),
        path_before: op.path_before.as_ref().map(|p| p.display().to_string()),
        path_after: op.path_after.as_ref().map(|p| p.display().to_string()),
        message: message.into(),
    }
}

pub(crate) fn apply_plan(
    plan: &WorkspaceApplyPlan,
    expected_hash: &str,
) -> Result<WorkspaceApplyReport> {
    if plan.validation_status != Some(ValidationStatus::Valid) {
        bail!("workspace apply requires plan validation_status=valid");
    }
    if plan.validation_proof.as_deref() != Some(expected_hash) {
        bail!("workspace apply requires validation_proof matching plan_hash");
    }
    let report = validate_plan(plan)?;
    if report.plan_hash != expected_hash {
        bail!("workspace plan hash mismatch");
    }
    let mut rollback = Vec::<(PathBuf, Option<String>)>::new();
    let mut journal = Vec::<RollbackEntry>::new();
    for op in &plan.operations {
        for path in [op.path_before.as_ref(), op.path_after.as_ref()]
            .into_iter()
            .flatten()
        {
            if !rollback.iter().any(|(p, _)| p == path) {
                let content = fs::read_to_string(path).ok();
                journal.push(RollbackEntry {
                    path: path.display().to_string(),
                    existed_before: content.is_some(),
                    content_sha256: content.as_ref().map(|c| sha256_hex(c)),
                });
                rollback.push((path.clone(), content));
            }
        }
    }
    let result = (|| -> Result<()> {
        for op in &plan.operations {
            match op.kind {
                WorkspaceOpKind::ModifyExact => fs::write(
                    op.path_before.as_ref().unwrap(),
                    operation_content(op)?.unwrap_or_default(),
                )?,
                WorkspaceOpKind::Add => {
                    if let Some(parent) = op.path_after.as_ref().unwrap().parent() {
                        if !parent.as_os_str().is_empty() {
                            fs::create_dir_all(parent)?;
                        }
                    }
                    fs::write(
                        op.path_after.as_ref().unwrap(),
                        operation_content(op)?.unwrap_or_default(),
                    )?;
                }
                WorkspaceOpKind::Delete => fs::remove_file(op.path_before.as_ref().unwrap())?,
                WorkspaceOpKind::Rename | WorkspaceOpKind::Move => {
                    if let Some(parent) = op.path_after.as_ref().unwrap().parent() {
                        if !parent.as_os_str().is_empty() {
                            fs::create_dir_all(parent)?;
                        }
                    }
                    fs::rename(
                        op.path_before.as_ref().unwrap(),
                        op.path_after.as_ref().unwrap(),
                    )?;
                }
                WorkspaceOpKind::ModifyFuzzy => {
                    let path = op.path_before.as_ref().unwrap();
                    let candidate = validate_fuzzy_candidate(plan, op, path)?;
                    fs::write(path, candidate)?;
                }
            }
        }
        Ok(())
    })();
    if let Err(err) = result {
        for (path, content) in rollback.into_iter().rev() {
            match content {
                Some(text) => {
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() {
                            let _ = fs::create_dir_all(parent);
                        }
                    }
                    let _ = fs::write(&path, text);
                }
                None => {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        return Err(err).context("workspace apply failed and rollback was attempted");
    }
    Ok(WorkspaceApplyReport {
        status: "applied".into(),
        plan_id: plan.plan_id.clone(),
        plan_hash: report.plan_hash,
        applied_operations: plan.operations.len(),
        rollback_journal: journal,
    })
}

pub(crate) fn refactor_plan(
    plan: &WorkspaceApplyPlan,
    chunk_size: usize,
) -> Result<RefactorPlanReport> {
    if chunk_size == 0 {
        bail!("chunk_size must be greater than zero");
    }
    let validation = validate_plan(plan)?;
    let mut chunks = Vec::new();
    for (idx, ops) in plan.operations.chunks(chunk_size).enumerate() {
        let mut paths = BTreeMap::<String, ()>::new();
        for op in ops {
            if let Some(path) = &op.path_before {
                paths.insert(path.display().to_string(), ());
            }
            if let Some(path) = &op.path_after {
                paths.insert(path.display().to_string(), ());
            }
        }
        chunks.push(RefactorChunk {
            chunk_id: format!("chunk-{:03}", idx + 1),
            operation_ids: ops.iter().map(|op| op.op_id.clone()).collect(),
            paths: paths.into_keys().collect(),
        });
    }
    Ok(RefactorPlanReport {
        status: "planned".into(),
        plan_id: validation.plan_id,
        chunk_size,
        chunk_count: chunks.len(),
        chunks,
        verification: vec![
            "validate all chunks before writes".into(),
            "apply one chunk at a time".into(),
            "run targeted verification after each chunk".into(),
            "rollback or stop on first conflict/failure".into(),
        ],
    })
}
