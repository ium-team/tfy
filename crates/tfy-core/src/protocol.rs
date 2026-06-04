use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FallbackAction {
    Selected,
    Related,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserMetadata {
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
    pub kind: String,
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResponse {
    pub root: String,
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
    pub reason: String,
    pub scopes: Vec<ScopeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMap {
    pub scope_id: String,
    pub symbols: BTreeMap<String, String>,
    pub reverse: BTreeMap<String, String>,
}

impl SymbolMap {
    pub fn new(scope_id: impl Into<String>, symbols: BTreeMap<String, String>) -> Self {
        let reverse = symbols
            .iter()
            .map(|(k, v)| (v.clone(), k.clone()))
            .collect();
        Self {
            scope_id: scope_id.into(),
            symbols,
            reverse,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub raw_chars: usize,
    pub compact_chars: usize,
    pub savings_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandResponse {
    pub scope: ScopeInfo,
    pub compactness: String,
    pub compact_code: String,
    pub symbol_map: SymbolMap,
    pub metrics: Metrics,
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullResponse {
    pub scope: ScopeInfo,
    pub code: String,
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePayload {
    pub scope_id: Option<String>,
    pub scope: Option<ScopeInfo>,
    pub compactness: Option<String>,
    pub compact_code: Option<String>,
    pub code: Option<String>,
    pub patch: Option<String>,
    pub language: Option<String>,
    pub symbols: Option<BTreeMap<String, String>>,
    pub reverse: Option<BTreeMap<String, String>>,
    pub symbol_map: Option<SymbolMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResponse {
    pub scope_id: String,
    pub restored_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDecision {
    pub action: FallbackAction,
    pub reason: String,
    pub parser: String,
    pub confidence: Confidence,
    pub fallback_action: FallbackAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Savings {
    pub raw_tokens: usize,
    pub compact_tokens: usize,
    pub saved_tokens: isize,
    pub savings_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub restoration_success: bool,
    pub fallback_action: String,
    pub missed_needed_code_failures: usize,
    pub missed_command_error_failures: usize,
    pub task_success_baseline: bool,
    pub task_success_compact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub savings: Savings,
    pub quality: QualityScore,
    pub release_gate: String,
}
