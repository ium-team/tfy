use crate::protocol::{Confidence, ContextDecision, FallbackAction};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

static TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b").expect("valid token regex"));
static COMPACT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:f\d+|[a-z]|v\d+)$").expect("valid compact regex"));
static UNRESOLVED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(NameError|ReferenceError|cannot find|not found|undefined|unresolved|E0425)")
        .expect("valid unresolved regex")
});
static NON_LOCAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(macro|dynamic dispatch|inheritance|global state|side effect|generated code)")
        .expect("valid non-local regex")
});
static SAFE_WORDS: Lazy<BTreeSet<&'static str>> = Lazy::new(|| {
    "def return for in if else let const var fn function class import export from async await true false none null nil this self"
        .split_whitespace()
        .collect()
});

pub fn decide_context_need(
    compact_code: &str,
    known_symbols: &BTreeMap<String, String>,
    diagnostics: &str,
) -> ContextDecision {
    let compact_names: BTreeSet<String> = known_symbols.values().cloned().collect();
    let mut reasons = Vec::new();
    let mut suspicious = Vec::new();
    let haystack = format!("{compact_code}\n{diagnostics}");
    for m in TOKEN.find_iter(&haystack) {
        let t = m.as_str();
        if COMPACT.is_match(t)
            && !compact_names.contains(t)
            && !SAFE_WORDS.contains(t)
            && !suspicious.iter().any(|x: &String| x == t)
        {
            suspicious.push(t.to_string());
        }
    }
    if !suspicious.is_empty() {
        reasons.push(format!(
            "unmapped compact symbols: {}",
            suspicious.into_iter().take(8).collect::<Vec<_>>().join(",")
        ));
    }
    if UNRESOLVED.is_match(diagnostics) {
        reasons.push("diagnostics mention unresolved or missing symbol".into());
    }
    if NON_LOCAL.is_match(diagnostics) {
        reasons.push("diagnostics mention non-local behavior".into());
    }
    let (action, confidence, reason) = if reasons.len() >= 2 {
        (FallbackAction::Full, Confidence::Low, reasons.join("; "))
    } else if !reasons.is_empty() {
        (
            FallbackAction::Related,
            Confidence::Medium,
            reasons.join("; "),
        )
    } else {
        (
            FallbackAction::Selected,
            Confidence::High,
            "selected compact scope appears sufficient".into(),
        )
    };
    ContextDecision {
        action: action.clone(),
        reason,
        parser: "policy".into(),
        confidence,
        fallback_action: action,
    }
}
