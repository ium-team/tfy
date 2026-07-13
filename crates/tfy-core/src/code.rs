use crate::language::LanguageKind;
use crate::protocol::*;
use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

static IDENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").expect("valid identifier regex"));
static COMPACT_SYM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:f\d+|[a-z]|v\d+)$").expect("valid compact symbol regex"));
static KEYWORDS: Lazy<BTreeSet<&'static str>> = Lazy::new(|| {
    "if else for while return break continue switch case default try catch finally throw new class struct enum interface import from export const let var def fn func function pub private protected static async await yield true false null none nil self this super in is and or not as with match where impl trait use mod crate type sizeof typeof do go package public void int float double char bool boolean string number any unknown never object undefined symbol bigint void elif except lambda global nonlocal pass raise assert del mut ref move unsafe extern dyn i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 str String Vec Option Result Box HashMap BTreeMap".split_whitespace().collect()
});

#[derive(Clone)]
struct ScopeInternal {
    info: ScopeInfo,
    start: usize,
    end: usize,
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn compact_context_ref(
    path: &str,
    byte_start: usize,
    byte_end: usize,
    source_sha256: &str,
    compactness: &str,
    compact: &str,
) -> String {
    let digest = sha256_hex(&format!(
        "{}:{}:{}:{}:{}:{}",
        path, byte_start, byte_end, source_sha256, compactness, compact
    ));
    format!("tfy_{}", &digest[..16])
}

fn proof_context_ref(
    scope: &ScopeInternal,
    source_slice: &str,
    compactness: &str,
    compact: &str,
) -> String {
    compact_context_ref(
        &scope.info.path,
        scope.start,
        scope.end,
        &sha256_hex(source_slice),
        compactness,
        compact,
    )
}

fn symbol_map_sha256(symbols: &BTreeMap<String, String>) -> String {
    let canonical = serde_json::to_string(symbols).expect("BTreeMap symbol map serializes");
    sha256_hex(&canonical)
}

fn build_apply_proof(
    scope: &ScopeInternal,
    full_source: &str,
    selected_source: &str,
    compactness: &str,
    compact: &str,
    symbols: &BTreeMap<String, String>,
) -> ApplyProof {
    ApplyProof {
        path: scope.info.path.clone(),
        scope_id: scope.info.id.clone(),
        language: scope.info.language.clone(),
        byte_start: scope.start,
        byte_end: scope.end,
        start_line: scope.info.start_line,
        end_line: scope.info.end_line,
        source_sha256: sha256_hex(selected_source),
        symbol_map_sha256: symbol_map_sha256(symbols),
        compact_code_sha256: sha256_hex(compact),
        file_sha256: sha256_hex(full_source),
        file_len: full_source.len(),
        compactness: compactness.to_string(),
        context_ref: proof_context_ref(scope, selected_source, compactness, compact),
        parser: scope.info.parser.clone(),
        confidence: scope.info.confidence.clone(),
        fallback_action: scope.info.fallback_action.clone(),
    }
}

pub fn index_path(path: impl AsRef<Path>) -> Result<IndexResponse> {
    let root = path.as_ref();
    let scopes = build_scopes(root)?;
    let meta = aggregate_meta(&scopes, root);
    Ok(IndexResponse {
        root: root.to_string_lossy().to_string(),
        parser: meta.parser,
        confidence: meta.confidence,
        fallback_action: meta.fallback_action,
        reason: meta.reason,
        scopes: scopes.into_iter().map(|s| s.info).collect(),
    })
}

pub fn expand_scope(
    path: impl AsRef<Path>,
    query: &str,
    compactness: &str,
) -> Result<ExpandResponse> {
    let root = path.as_ref();
    let scope = find_scope(root, query)?;
    let source = fs::read_to_string(&scope.info.path)?;
    let code = source
        .get(scope.start..scope.end)
        .ok_or_else(|| anyhow!("invalid scope byte range"))?;
    let lang = LanguageKind::from_path(Path::new(&scope.info.path));
    let compactness = compactness.to_ascii_lowercase();
    if compactness != "light" && compactness != "symbol" {
        bail!("compactness must be light or symbol");
    }
    if compactness == "symbol" && matches!(scope.info.confidence, Confidence::Low) {
        bail!("symbol-mode declined for low-confidence parser result; request full context");
    }
    let symbols = if compactness == "symbol" {
        build_symbol_map(&scope.info, code, lang)
    } else {
        BTreeMap::new()
    };
    let mut compact = if compactness == "symbol" {
        replace_symbols(code, &symbols, lang)
    } else {
        code.to_string()
    };
    compact = if lang == LanguageKind::Python {
        minify_python_layout(&compact)
    } else {
        minify_non_python(&compact)
    };
    let raw_chars = code.len();
    let compact_chars = compact.len();
    let savings_pct = if raw_chars == 0 {
        0.0
    } else {
        ((raw_chars as f64 - compact_chars as f64) / raw_chars as f64 * 10000.0).round() / 100.0
    };
    let apply_proof = build_apply_proof(&scope, &source, code, &compactness, &compact, &symbols);
    Ok(ExpandResponse {
        parser: scope.info.parser.clone(),
        confidence: scope.info.confidence.clone(),
        fallback_action: scope.info.fallback_action.clone(),
        reason: scope.info.reason.clone(),
        scope: scope.info.clone(),
        apply_proof: Some(apply_proof),
        compactness,
        compact_code: compact,
        symbol_map: SymbolMap::new(scope.info.id.clone(), symbols),
        metrics: Metrics {
            raw_chars,
            compact_chars,
            savings_pct,
        },
    })
}

pub fn full_scope(path: impl AsRef<Path>, query: &str) -> Result<FullResponse> {
    let root = path.as_ref();
    let scope = find_scope(root, query)?;
    let source = fs::read_to_string(&scope.info.path)?;
    let code = source
        .get(scope.start..scope.end)
        .ok_or_else(|| anyhow!("invalid scope byte range"))?
        .to_string();
    Ok(FullResponse {
        parser: scope.info.parser.clone(),
        confidence: scope.info.confidence.clone(),
        fallback_action: scope.info.fallback_action.clone(),
        reason: scope.info.reason.clone(),
        scope: scope.info,
        code,
    })
}

pub fn restore_payload(payload: RestorePayload) -> Result<RestoreResponse> {
    let scope_id = payload
        .scope_id
        .clone()
        .or_else(|| payload.scope.as_ref().map(|s| s.id.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let lang_name = payload
        .language
        .clone()
        .or_else(|| payload.scope.as_ref().map(|s| s.language.clone()))
        .unwrap_or_else(|| "python".to_string());
    let lang = match lang_name.as_str() {
        "python" => LanguageKind::Python,
        "javascript" => LanguageKind::JavaScript,
        "jsx" => LanguageKind::Jsx,
        "typescript" => LanguageKind::TypeScript,
        "tsx" => LanguageKind::Tsx,
        "rust" => LanguageKind::Rust,
        "go" => LanguageKind::Go,
        _ => LanguageKind::Generic,
    };
    let compactness = payload.compactness.clone().unwrap_or_else(|| {
        if payload
            .symbol_map
            .as_ref()
            .map(|m| m.symbols.is_empty())
            .unwrap_or(true)
        {
            "light".into()
        } else {
            "symbol".into()
        }
    });
    let compact = payload
        .compact_code
        .or(payload.code)
        .or(payload.patch)
        .unwrap_or_default();
    let symbols = if let Some(map) = payload.symbol_map {
        map.symbols
    } else if let Some(symbols) = payload.symbols {
        symbols
    } else if let Some(reverse) = payload.reverse {
        reverse
            .into_iter()
            .map(|(short, original)| (original, short))
            .collect()
    } else {
        BTreeMap::new()
    };
    if compactness == "light" && symbols.is_empty() {
        return Ok(RestoreResponse {
            scope_id,
            restored_code: format!("{}\n", compact.trim()),
        });
    }
    let reverse: BTreeMap<String, String> = symbols
        .iter()
        .map(|(k, v)| (v.clone(), k.clone()))
        .collect();
    reject_unmapped(&compact, &reverse, lang)?;
    let restored = replace_symbols(&compact, &reverse, lang);
    Ok(RestoreResponse {
        scope_id,
        restored_code: format!("{}\n", restored.trim()),
    })
}

pub fn restore_display_payload(payload: RestorePayload) -> Result<RestoreDisplayResponse> {
    let restored = restore_payload(payload)?;
    let (display_code, warning) = readable_display_code(&restored.restored_code);
    Ok(RestoreDisplayResponse {
        scope_id: restored.scope_id,
        restored_code: restored.restored_code,
        display_code,
        display_only: true,
        authority: "display_only_not_apply_authority".into(),
        warning,
    })
}

pub fn restore_file_payload(payload: RestorePayload) -> Result<RestoreFileResponse> {
    let restored = restore_payload(payload.clone())?;
    let (file_code, warning) = readable_display_code(&restored.restored_code);
    let audit_symbols = payload
        .symbol_map
        .as_ref()
        .map(|m| &m.symbols)
        .or(payload.symbols.as_ref());
    Ok(RestoreFileResponse {
        scope_id: restored.scope_id,
        restored_code: restored.restored_code,
        file_code: file_code.clone(),
        canonical_for: "file_write_and_user_display".into(),
        compact_transport_only: true,
        symbol_audit_hash: format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_string(&audit_symbols)
                    .unwrap_or_default()
                    .as_bytes()
            )
        ),
        warning,
    })
}

pub fn restore_patch_payload(payload: RestorePayload) -> Result<RestoreFileResponse> {
    restore_file_payload(payload)
}

fn readable_display_code(code: &str) -> (String, Option<String>) {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return ("\n".into(), Some("restored code is empty".into()));
    }
    if trimmed.contains('\n') && !trimmed.lines().any(|line| line.len() > 160) {
        return (format!("{}\n", trimmed), None);
    }
    let mut out = String::new();
    let mut indent = 0usize;
    let mut in_string: Option<char> = None;
    let mut escape = false;
    let write_indent = |out: &mut String, indent: usize| {
        for _ in 0..indent {
            out.push_str("  ");
        }
    };
    for ch in trimmed.chars() {
        if let Some(quote) = in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => {
                in_string = Some(ch);
                out.push(ch);
            }
            '{' | '[' | '(' => {
                out.push(ch);
                out.push('\n');
                indent += 1;
                write_indent(&mut out, indent);
            }
            '}' | ']' | ')' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                indent = indent.saturating_sub(1);
                write_indent(&mut out, indent);
                out.push(ch);
            }
            ';' => {
                out.push(';');
                out.push('\n');
                write_indent(&mut out, indent);
            }
            ',' => {
                out.push(',');
                if indent > 0 {
                    out.push('\n');
                    write_indent(&mut out, indent);
                } else {
                    out.push(' ');
                }
            }
            _ => out.push(ch),
        }
    }
    let display = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    (format!("{}\n", display.trim()), None)
}

pub fn apply_restored_payload(
    payload: RestorePayload,
    proof_override: Option<ApplyProof>,
) -> Result<ApplyResult> {
    let proof = match (payload.apply_proof.clone(), proof_override) {
        (Some(_), Some(_)) => bail!("output-gateway apply requires exactly one apply proof source"),
        (Some(proof), None) | (None, Some(proof)) => proof,
        (None, None) => bail!("output-gateway apply requires content-addressed apply proof"),
    };
    if proof.path.is_empty() || proof.scope_id.is_empty() || proof.context_ref.is_empty() {
        bail!("apply proof is missing required identity fields");
    }
    if proof.byte_start >= proof.byte_end {
        bail!("apply proof has invalid byte range");
    }
    if matches!(proof.confidence, Confidence::Low)
        || matches!(proof.fallback_action, FallbackAction::Full)
    {
        bail!("apply proof is not authoritative for workspace writes");
    }
    let payload_scope_id = payload
        .scope_id
        .clone()
        .or_else(|| payload.scope.as_ref().map(|s| s.id.clone()))
        .unwrap_or_else(|| proof.scope_id.clone());
    if payload_scope_id != proof.scope_id {
        bail!("restore payload scope does not match apply proof");
    }
    let compactness = payload
        .compactness
        .clone()
        .unwrap_or_else(|| proof.compactness.clone());
    if compactness != proof.compactness {
        bail!("restore payload compactness does not match apply proof");
    }
    let language = payload
        .language
        .clone()
        .or_else(|| payload.scope.as_ref().map(|s| s.language.clone()))
        .unwrap_or_else(|| proof.language.clone());
    if language != proof.language {
        bail!("restore payload language does not match apply proof");
    }
    if proof.compactness == "symbol" {
        let Some(symbol_map) = payload.symbol_map.as_ref() else {
            bail!("symbol-mode apply requires a scoped symbol map");
        };
        if symbol_map.scope_id != proof.scope_id {
            bail!("restore payload symbol map does not match apply proof scope");
        }
        if symbol_map_sha256(&symbol_map.symbols) != proof.symbol_map_sha256 {
            bail!("restore payload symbol map does not match apply proof content");
        }
        if payload.symbols.is_some() || payload.reverse.is_some() {
            bail!("symbol-mode apply requires exactly one scoped symbol_map authority");
        }
    }
    let Some(payload_context_ref) = payload.context_ref.as_ref() else {
        bail!("apply payload requires source context_ref matching apply proof");
    };
    if payload_context_ref != &proof.context_ref {
        bail!("apply payload context_ref does not match apply proof");
    }
    let Some(base_compact_code) = payload.base_compact_code.as_ref() else {
        bail!("apply payload requires base_compact_code matching apply proof");
    };
    let base_compact_sha256 = sha256_hex(base_compact_code);
    if base_compact_sha256 != proof.compact_code_sha256 {
        bail!("apply payload base compact code does not match apply proof");
    }
    let expected_context_ref = compact_context_ref(
        &proof.path,
        proof.byte_start,
        proof.byte_end,
        &proof.source_sha256,
        &proof.compactness,
        base_compact_code,
    );
    if expected_context_ref != proof.context_ref {
        bail!("apply proof context_ref does not match base compact context");
    }
    let restored = restore_payload(payload)?;
    if restored.restored_code.trim().is_empty() {
        bail!("empty restored code is not applied; deletion semantics are not implemented");
    }
    let path = Path::new(&proof.path);
    let current = fs::read_to_string(path)
        .with_context(|| format!("read apply target {}", path.display()))?;
    if proof.byte_end > current.len()
        || !current.is_char_boundary(proof.byte_start)
        || !current.is_char_boundary(proof.byte_end)
    {
        bail!("apply proof byte range is stale or invalid");
    }
    let selected = current
        .get(proof.byte_start..proof.byte_end)
        .ok_or_else(|| anyhow!("apply proof byte range is not valid utf-8 boundary"))?;
    if sha256_hex(selected) != proof.source_sha256 {
        bail!("apply proof source hash is stale");
    }
    // Whole-file hash/length in ApplyProof are supplemental stale signals.
    // The authoritative write gate is the captured byte range plus selected-source hash.
    let lang = LanguageKind::from_path(path);
    ensure_supported_parse(&current, lang, "current file")?;
    let mut next =
        String::with_capacity(current.len() - selected.len() + restored.restored_code.len());
    next.push_str(&current[..proof.byte_start]);
    next.push_str(&restored.restored_code);
    next.push_str(&current[proof.byte_end..]);
    ensure_supported_parse(&next, lang, "restored file")?;

    let before_sha256 = sha256_hex(&current);
    let after_sha256 = sha256_hex(&next);
    let metadata = fs::metadata(path)
        .with_context(|| format!("read apply target metadata {}", path.display()))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temp file in {}", parent.display()))?;
    tmp.write_all(next.as_bytes())?;
    tmp.flush()?;
    tmp.as_file_mut().sync_all()?;
    fs::set_permissions(tmp.path(), metadata.permissions())
        .with_context(|| format!("preserve permissions for {}", path.display()))?;
    tmp.persist(path)
        .map_err(|e| anyhow!("persist apply target {}: {}", path.display(), e.error))?;
    let restored_len = restored.restored_code.len();
    let patch_ref = format!("tfy_{}", &sha256_hex(&restored.restored_code)[..16]);
    Ok(ApplyResult {
        scope_id: proof.scope_id,
        path: proof.path,
        restored_code: restored.restored_code,
        applied: true,
        byte_start: proof.byte_start,
        byte_end: proof.byte_start + restored_len,
        before_sha256,
        after_sha256,
        patch_ref,
        context_ref: proof.context_ref,
    })
}

fn ensure_supported_parse(source: &str, lang: LanguageKind, label: &str) -> Result<()> {
    let Some(ts_lang) = lang.tree_sitter() else {
        bail!("{label} uses unsupported language for authoritative apply");
    };
    let mut parser = Parser::new();
    parser
        .set_language(&ts_lang)
        .context("set tree-sitter language")?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("parse failed"))?;
    if tree.root_node().has_error() {
        bail!("{label} does not parse cleanly after apply");
    }
    Ok(())
}

fn aggregate_meta(scopes: &[ScopeInternal], root: &Path) -> ParserMetadata {
    if scopes.is_empty() {
        return LanguageKind::from_path(root).metadata(
            false,
            LanguageKind::from_path(root) == LanguageKind::CFamily
                || LanguageKind::from_path(root) == LanguageKind::Generic,
        );
    }
    let low = scopes
        .iter()
        .any(|s| matches!(s.info.confidence, Confidence::Low));
    let med = scopes
        .iter()
        .any(|s| matches!(s.info.confidence, Confidence::Medium));
    if low {
        ParserMetadata {
            parser: "mixed".into(),
            confidence: Confidence::Low,
            fallback_action: FallbackAction::Full,
            reason: "one or more files are low-confidence".into(),
        }
    } else if med {
        ParserMetadata {
            parser: "mixed".into(),
            confidence: Confidence::Medium,
            fallback_action: FallbackAction::Related,
            reason: "one or more files use syntax recovery".into(),
        }
    } else {
        ParserMetadata {
            parser: "mixed".into(),
            confidence: Confidence::High,
            fallback_action: FallbackAction::Selected,
            reason: "all indexed files parsed with production adapters".into(),
        }
    }
}

fn build_scopes(root: &Path) -> Result<Vec<ScopeInternal>> {
    let mut out = Vec::new();
    for path in iter_files(root)? {
        let lang = LanguageKind::from_path(&path);
        if lang == LanguageKind::Generic {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        let display = if root.is_dir() {
            path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string()
        } else {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };
        out.extend(extract_file_scopes(&path, &display, &text, lang)?);
    }
    Ok(out)
}

fn iter_files(root: &Path) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if [
                ".git",
                ".omx",
                "target",
                "node_modules",
                "__pycache__",
                ".venv",
                "dist",
                "build",
            ]
            .contains(&name)
            {
                continue;
            }
            if path.is_dir() {
                walk(&path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }
    walk(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn extract_file_scopes(
    path: &Path,
    display: &str,
    text: &str,
    lang: LanguageKind,
) -> Result<Vec<ScopeInternal>> {
    let experimental = lang == LanguageKind::CFamily || lang == LanguageKind::Generic;
    let Some(ts_lang) = lang.tree_sitter() else {
        return Ok(vec![file_scope(path, display, text, lang, true)]);
    };
    let mut parser = Parser::new();
    parser
        .set_language(&ts_lang)
        .context("set tree-sitter language")?;
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow!("parse failed"))?;
    let root_has_error = tree.root_node().has_error();
    let root_meta = lang.metadata(root_has_error, experimental);
    let mut scopes = Vec::new();
    let ctx = ScopeWalkCtx {
        text,
        path,
        display,
        lang,
        root_has_error,
        root_meta: &root_meta,
    };
    collect_scopes(tree.root_node(), &ctx, &mut scopes);
    if scopes.is_empty() {
        scopes.push(file_scope_with_meta(path, display, text, lang, root_meta));
    }
    Ok(scopes)
}

fn file_scope(
    path: &Path,
    display: &str,
    text: &str,
    lang: LanguageKind,
    experimental: bool,
) -> ScopeInternal {
    file_scope_with_meta(
        path,
        display,
        text,
        lang,
        lang.metadata(false, experimental),
    )
}
fn file_scope_with_meta(
    path: &Path,
    display: &str,
    text: &str,
    lang: LanguageKind,
    meta: ParserMetadata,
) -> ScopeInternal {
    let name = Path::new(display)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ScopeInternal {
        start: 0,
        end: text.len(),
        info: ScopeInfo {
            id: format!("{}:file:1", display.replace('\\', "/")),
            name,
            path: path.to_string_lossy().to_string(),
            language: lang.name().into(),
            start_line: 1,
            end_line: line_for_byte(text, text.len()),
            byte_start: 0,
            byte_end: text.len(),
            kind: "file".into(),
            parser: meta.parser,
            confidence: meta.confidence,
            fallback_action: meta.fallback_action,
            reason: meta.reason,
        },
    }
}

struct ScopeWalkCtx<'a> {
    text: &'a str,
    path: &'a Path,
    display: &'a str,
    lang: LanguageKind,
    root_has_error: bool,
    root_meta: &'a ParserMetadata,
}

fn collect_scopes(node: Node, ctx: &ScopeWalkCtx<'_>, out: &mut Vec<ScopeInternal>) {
    if ctx.lang.scope_kinds().contains(&node.kind()) {
        if let Some(name) = scope_name(node, ctx.text, ctx.lang) {
            let start = node.start_byte();
            let end = node.end_byte();
            let line = node.start_position().row + 1;
            let scope_meta = scope_metadata(
                ctx.lang,
                ctx.root_has_error,
                node.has_error(),
                ctx.root_meta,
            );
            out.push(ScopeInternal {
                start,
                end,
                info: ScopeInfo {
                    id: format!("{}:{}:{}", ctx.display.replace('\\', "/"), name, line),
                    name,
                    path: ctx.path.to_string_lossy().to_string(),
                    language: ctx.lang.name().into(),
                    start_line: line,
                    end_line: node.end_position().row + 1,
                    byte_start: start,
                    byte_end: end,
                    kind: node.kind().into(),
                    parser: scope_meta.parser,
                    confidence: scope_meta.confidence,
                    fallback_action: scope_meta.fallback_action,
                    reason: scope_meta.reason,
                },
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_scopes(child, ctx, out);
    }
}

fn scope_metadata(
    lang: LanguageKind,
    root_has_error: bool,
    scope_has_error: bool,
    root_meta: &ParserMetadata,
) -> ParserMetadata {
    if scope_has_error {
        return lang.metadata(true, false);
    }
    if root_has_error {
        return ParserMetadata {
            parser: lang.parser_name(),
            confidence: Confidence::Medium,
            fallback_action: FallbackAction::Related,
            reason:
                "tree-sitter found syntax errors outside this scope; related context recommended"
                    .into(),
        };
    }
    root_meta.clone()
}

fn scope_name(node: Node, text: &str, lang: LanguageKind) -> Option<String> {
    if let Some(n) = node.child_by_field_name("name") {
        return n.utf8_text(text.as_bytes()).ok().map(|s| s.to_string());
    }
    if matches!(node.kind(), "arrow_function" | "function") {
        return Some(format!("anonymous_{}", node.start_position().row + 1));
    }
    if lang == LanguageKind::Go && node.kind() == "type_declaration" {
        return Some(format!("type_{}", node.start_position().row + 1));
    }
    None
}

fn find_scope(root: &Path, query: &str) -> Result<ScopeInternal> {
    let scopes = build_scopes(root)?;
    if let Some(s) = scopes.iter().find(|s| s.info.id == query) {
        return Ok(s.clone());
    }
    let matches: Vec<_> = scopes
        .into_iter()
        .filter(|s| s.info.name == query)
        .collect();
    let mut matches = matches.into_iter();
    let Some(first) = matches.next() else {
        bail!("scope not found: {query}");
    };
    if matches.next().is_some() {
        bail!("ambiguous scope name '{query}'; use exact scope id");
    }
    Ok(first)
}

fn line_for_byte(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn build_symbol_map(scope: &ScopeInfo, code: &str, lang: LanguageKind) -> BTreeMap<String, String> {
    let mut names = Vec::new();
    if scope.kind != "file" && renamable(&scope.name) {
        names.push(scope.name.clone());
    }
    for name in identifier_tokens(code, lang) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let mut out = BTreeMap::new();
    let mut i = 0;
    for name in names {
        if name == scope.name && scope.kind != "file" {
            out.insert(name, "f1".into());
        } else {
            out.insert(name, short_symbol(i));
            i += 1;
        }
    }
    out
}

fn short_symbol(i: usize) -> String {
    let alpha = b"abcdefghijklmnopqrstuvwxyz";
    if i < 26 {
        (alpha[i] as char).to_string()
    } else {
        format!("v{}", i - 25)
    }
}
fn renamable(s: &str) -> bool {
    IDENT.is_match(s)
        && !KEYWORDS.contains(s)
        && ![
            "self", "this", "super", "true", "false", "null", "None", "True", "False",
        ]
        .contains(&s)
}

fn identifier_tokens(code: &str, lang: LanguageKind) -> Vec<String> {
    let masked = mask_strings_comments(code, lang);
    let mut out = Vec::new();
    for m in IDENT.find_iter(&masked) {
        let t = m.as_str();
        let prev = masked[..m.start()]
            .chars()
            .rev()
            .find(|c| !c.is_whitespace());
        let next = masked[m.end()..].chars().find(|c| !c.is_whitespace());
        if prev == Some('.')
            || (lang == LanguageKind::Rust
                && (matches!(next, Some('!') | Some('#'))
                    || is_rust_literal_prefix(code, m.start(), m.end())))
        {
            continue;
        }
        if renamable(t) && !out.iter().any(|x: &String| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

fn replace_symbols(code: &str, map: &BTreeMap<String, String>, lang: LanguageKind) -> String {
    if map.is_empty() {
        return code.to_string();
    }
    let masked = mask_strings_comments(code, lang);
    let mut out = String::new();
    let mut last = 0;
    for m in IDENT.find_iter(&masked) {
        out.push_str(&code[last..m.start()]);
        let prev = masked[..m.start()]
            .chars()
            .rev()
            .find(|c| !c.is_whitespace());
        let next = masked[m.end()..].chars().find(|c| !c.is_whitespace());
        if prev != Some('.')
            && !(lang == LanguageKind::Rust
                && (matches!(next, Some('!') | Some('#'))
                    || is_rust_literal_prefix(code, m.start(), m.end())))
        {
            if let Some(rep) = map.get(m.as_str()) {
                out.push_str(rep);
            } else {
                out.push_str(&code[m.start()..m.end()]);
            }
        } else {
            out.push_str(&code[m.start()..m.end()]);
        }
        last = m.end();
    }
    out.push_str(&code[last..]);
    out
}

fn reject_unmapped(
    code: &str,
    reverse: &BTreeMap<String, String>,
    lang: LanguageKind,
) -> Result<()> {
    for token in identifier_tokens(code, lang) {
        if COMPACT_SYM.is_match(&token)
            && !reverse.contains_key(&token)
            && !KEYWORDS.contains(token.as_str())
        {
            bail!("unmapped compact symbol: {token}");
        }
    }
    Ok(())
}

fn mask_strings_comments(code: &str, lang: LanguageKind) -> String {
    let mut bytes = code.as_bytes().to_vec();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            while i < len && bytes[i] != b'\n' {
                bytes[i] = b' ';
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            bytes[i] = b' ';
            bytes[i + 1] = b' ';
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                bytes[i] = b' ';
                i += 1;
            }
            if i + 1 < len {
                bytes[i] = b' ';
                bytes[i + 1] = b' ';
                i += 2;
            }
            continue;
        }
        if bytes[i] == b'#' && lang == LanguageKind::Python {
            while i < len && bytes[i] != b'\n' {
                bytes[i] = b' ';
                i += 1;
            }
            continue;
        }
        if lang == LanguageKind::Rust {
            if let Some(end) = rust_raw_string_end(&bytes, i) {
                for b in &mut bytes[i..end] {
                    *b = b' ';
                }
                i = end;
                continue;
            }
        }
        if bytes[i] == b'\'' && lang == LanguageKind::Rust && is_rust_lifetime(&bytes, i) {
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            continue;
        }
        if b"'\"`".contains(&bytes[i]) {
            let q = bytes[i];
            bytes[i] = b' ';
            i += 1;
            let mut esc = false;
            while i < len {
                let c = bytes[i];
                bytes[i] = b' ';
                if esc {
                    esc = false;
                } else if c == b'\\' {
                    esc = true;
                } else if c == q {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    String::from_utf8(bytes)
        .expect("mask preserves valid utf-8 by replacing masked bytes with ASCII spaces")
}

fn rust_raw_string_end(bytes: &[u8], i: usize) -> Option<usize> {
    let len = bytes.len();
    let mut j = i;
    if matches!(bytes.get(j), Some(b'b') | Some(b'c')) {
        j += 1;
    }
    if bytes.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0usize;
    while bytes.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if bytes.get(j) != Some(&b'"') {
        return None;
    }
    j += 1;
    while j < len {
        if bytes[j] == b'"' {
            let mut k = j + 1;
            let mut seen = 0usize;
            while seen < hashes && bytes.get(k) == Some(&b'#') {
                seen += 1;
                k += 1;
            }
            if seen == hashes {
                return Some(k);
            }
        }
        j += 1;
    }
    Some(len)
}

fn is_rust_literal_prefix(masked: &str, start: usize, end: usize) -> bool {
    let token = &masked[start..end];
    if !matches!(token, "r" | "b" | "c" | "br" | "rb" | "cr" | "rc") {
        return false;
    }
    let prev = masked[..start].chars().rev().find(|c| !c.is_whitespace());
    if matches!(prev, Some(c) if c.is_alphanumeric() || c == '_') {
        return false;
    }
    let tail = &masked[end..];
    let mut chars = tail.chars().skip_while(|c| c.is_whitespace());
    match token {
        "b" | "c" => chars.next() == Some('"'),
        "r" => {
            let first = chars.next();
            first == Some('"') || first == Some('#')
        }
        "br" | "rb" | "cr" | "rc" => {
            let first = chars.next();
            first == Some('"') || first == Some('#')
        }
        _ => false,
    }
}

fn is_rust_lifetime(bytes: &[u8], i: usize) -> bool {
    if bytes.get(i) != Some(&b'\'') {
        return false;
    }
    let Some(&next) = bytes.get(i + 1) else {
        return false;
    };
    if !(next.is_ascii_alphabetic() || next == b'_') {
        return false;
    }
    let after = bytes.get(i + 2).copied();
    if after == Some(b'\'') {
        return false;
    }
    let prev = bytes[..i]
        .iter()
        .rev()
        .copied()
        .find(|b| !b.is_ascii_whitespace());
    !matches!(prev, Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'))
}

fn minify_python_layout(code: &str) -> String {
    code.lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
fn minify_non_python(code: &str) -> String {
    let mut out = String::new();
    let mut in_str = None;
    let mut esc = false;
    let mut pending = false;
    let ops: BTreeSet<char> = "{}()[],;:+-*/=<>".chars().collect();
    let chars: Vec<char> = code.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let n = chars.get(i + 1).copied().unwrap_or('\0');
        if let Some(q) = in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if c == '/' && n == '/' {
            if pending && !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            while i < chars.len() && chars[i] != '\n' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == '\n' {
                out.push('\n');
                i += 1;
            }
            pending = false;
            continue;
        }
        if c == '/' && n == '*' {
            if pending && !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            out.push(chars[i]);
            out.push(chars[i + 1]);
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                out.push(chars[i]);
                i += 1;
            }
            if i + 1 < chars.len() {
                out.push(chars[i]);
                out.push(chars[i + 1]);
                i += 2;
            }
            pending = true;
            continue;
        }
        if ['\'', '"', '`'].contains(&c) {
            if pending && need_space(out.chars().last(), c) {
                out.push(' ');
            }
            pending = false;
            in_str = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            pending = true;
            i += 1;
            continue;
        }
        if ops.contains(&c) {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
            pending = false;
            i += 1;
            continue;
        }
        if pending && need_space(out.chars().last(), c) {
            out.push(' ');
        }
        pending = false;
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}
fn need_space(prev: Option<char>, c: char) -> bool {
    prev.map(|p| {
        (p.is_alphanumeric() || p == '_' || p == '$')
            && (c.is_alphanumeric() || c == '_' || c == '$')
    })
    .unwrap_or(false)
}
