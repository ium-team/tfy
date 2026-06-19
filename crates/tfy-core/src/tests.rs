use crate::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn tree_sitter_index_has_mandatory_metadata() {
    let response = index_path(root().join("examples/sample.py")).unwrap();
    assert_eq!(response.confidence, Confidence::High);
    assert_eq!(response.fallback_action, FallbackAction::Selected);
    assert!(!response.parser.is_empty());
    assert!(!response.reason.is_empty());
    assert!(response
        .scopes
        .iter()
        .any(|s| s.name == "calculate_total_price"));
}

#[test]
fn expand_restore_python_symbol_round_trip() {
    let exp = expand_scope(
        root().join("examples/sample.py"),
        "sample.py:calculate_total_price:1",
        "symbol",
    )
    .unwrap();
    assert!(exp.compact_code.contains("def f1"));
    assert_eq!(exp.confidence, Confidence::High);
    let payload = RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        compact_code: Some(exp.compact_code.clone()),
        base_compact_code: None,
        context_ref: None,
        code: None,
        patch: None,
        language: Some("python".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map.clone()),
        apply_proof: None,
    };
    let restored = restore_payload(payload).unwrap().restored_code;
    assert!(restored.contains("calculate_total_price"));
    assert!(restored.contains("tax_rate"));
}

#[test]
fn first_wave_language_corpus_meets_minimum_and_indexes() {
    let corpus = root().join("corpus");
    for (dir, min_files) in [
        ("python", 25usize),
        ("javascript", 25),
        ("jsx", 25),
        ("typescript", 25),
        ("tsx", 25),
        ("rust", 25),
        ("go", 25),
    ] {
        let lang_dir = corpus.join(dir);
        let files = std::fs::read_dir(&lang_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count();
        assert!(files >= min_files, "{dir} has {files} corpus files");
        let idx = index_path(&lang_dir).unwrap();
        assert_eq!(idx.confidence, Confidence::High, "{dir}");
        assert!(idx.scopes.len() >= min_files, "{dir}");
    }
}

#[test]
fn non_python_literal_contents_are_preserved() {
    let exp = expand_scope(
        root().join("corpus/javascript/sample.js"),
        "sample.js:greet:1",
        "symbol",
    )
    .unwrap();
    assert!(exp.compact_code.contains("\"hello : \""));
    let payload = RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        compact_code: Some(exp.compact_code.clone()),
        base_compact_code: None,
        context_ref: None,
        code: None,
        patch: None,
        language: Some("javascript".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map.clone()),
        apply_proof: None,
    };
    let restored = restore_payload(payload).unwrap().restored_code;
    assert!(restored.contains("\"hello : \""));
}

#[test]
fn command_feedback_launch_failure_has_raw_ref() {
    let dir = tempfile::tempdir().unwrap();
    let summary = run_command(
        &["does-not-exist-tfy-rust".to_string()],
        None,
        dir.path(),
        64,
    )
    .unwrap();
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.exit_code, 127);
    assert!(summary.summary.contains("raw_ref="));
    let raw = raw_output(dir.path(), &summary.raw_ref, None, 3).unwrap();
    assert!(raw.contains("command launch failed"));
}

#[test]
fn command_feedback_prioritizes_tail_error_after_file_refs() {
    let dir = tempfile::tempdir().unwrap();
    let store = RawStore::new(dir.path()).unwrap();
    let refs = (0..13)
        .map(|i| format!("src/noise{i}.py:1: note\n"))
        .collect::<String>();
    let raw = refs + "tail.py:99: error: broken\n";
    let rf = store.put("raw", &raw, 1).unwrap();
    assert!(store
        .raw(&rf, Some("tail.py:99"), 1)
        .unwrap()
        .contains("tail.py:99"));
    let summary = run_command(
        &["python3".into(), "-c".into(), "print('ok')".into()],
        None,
        dir.path(),
        64,
    )
    .unwrap();
    assert_eq!(summary.summary, "ok\n");
    assert!(!summary.summary.contains("raw_ref="));
}

#[test]
fn raw_ref_returns_exact_captured_output_without_tfy_notice() {
    let dir = tempfile::tempdir().unwrap();
    let summary = run_command(
        &["python3".into(), "-c".into(), "print('A' * 200)".into()],
        None,
        dir.path(),
        10,
    )
    .unwrap();
    let raw = raw_output(dir.path(), &summary.raw_ref, None, 3).unwrap();
    assert_eq!(raw, format!("{}\n", "A".repeat(200)));
    assert!(!raw.contains("tfy:"));
}

#[test]
fn syntax_error_uses_low_full_and_declines_symbol_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.py");
    std::fs::write(
        &path,
        "def broken(value):\n    if value >\n        return value\n",
    )
    .unwrap();
    let idx = index_path(&path).unwrap();
    assert_eq!(idx.confidence, Confidence::Low);
    assert_eq!(idx.fallback_action, FallbackAction::Full);
    let err = expand_scope(&path, "broken", "symbol")
        .unwrap_err()
        .to_string();
    assert!(err.contains("symbol-mode declined"), "{err}");
}

#[test]
fn symbol_map_does_not_rename_language_primitives() {
    let ts = expand_scope(
        root().join("corpus/typescript/sample.ts"),
        "sample.ts:addItems:1",
        "symbol",
    )
    .unwrap();
    assert!(!ts.symbol_map.symbols.contains_key("number"));
    assert!(ts.compact_code.contains("number"));
    let rs = expand_scope(
        root().join("corpus/rust/fixture_01.rs"),
        "fixture_01.rs:calculate_discount_1:5",
        "symbol",
    )
    .unwrap();
    assert!(!rs.symbol_map.symbols.contains_key("f64"));
    assert!(rs.compact_code.contains("f64"));
}

#[test]
fn non_python_comments_and_literals_survive_compact_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("comments.js");
    std::fs::write(
        &path,
        "function keepComments(value) {\n  // do not drop me\n  const text = \"a : b\";\n  /* keep block */\n  return text + value;\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "keepComments", "symbol").unwrap();
    assert!(exp.compact_code.contains("// do not drop me\n"));
    assert!(exp.compact_code.contains("/* keep block */"));
    assert!(exp.compact_code.contains("\"a : b\""));
    let restored = restore_payload(RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        compact_code: Some(exp.compact_code.clone()),
        base_compact_code: None,
        context_ref: None,
        code: None,
        patch: None,
        language: Some("javascript".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map.clone()),
        apply_proof: None,
    })
    .unwrap()
    .restored_code;
    assert!(restored.contains("// do not drop me\n"));
    assert!(restored.contains("/* keep block */"));
    assert!(restored.contains("\"a : b\""));
}

#[test]
fn decide_context_can_return_related() {
    let mut symbols = std::collections::BTreeMap::new();
    symbols.insert("calculate_total_price".to_string(), "f1".to_string());
    let decision = decide_context_need("def f1(a): return z", &symbols, "");
    assert_eq!(decision.action, FallbackAction::Related);
}

#[test]
fn rust_oracle_fixtures_include_required_metadata() {
    let fixture =
        std::fs::read_to_string(root().join("oracle/fixtures/rust-index-sample.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    for key in ["parser", "confidence", "fallback_action", "reason"] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
    let scopes = v["scopes"].as_array().unwrap();
    assert!(!scopes.is_empty());
    for scope in scopes {
        for key in ["parser", "confidence", "fallback_action", "reason"] {
            assert!(scope.get(key).is_some(), "scope missing {key}");
        }
    }
}

#[test]
fn unicode_literals_do_not_corrupt_symbol_replacement_offsets() {
    let exp = expand_scope(
        root().join("corpus/javascript/hostile_template_unicode.js"),
        "hostile_template_unicode.js:unicodeTemplate:1",
        "symbol",
    )
    .unwrap();
    assert!(exp.compact_code.contains("hé 😀"));
    assert!(!exp.compact_code.contains("retb"));
    let restored = restore_payload(RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        base_compact_code: None,
        context_ref: None,
        compact_code: Some(exp.compact_code),
        code: None,
        patch: None,
        language: Some("javascript".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map),
        apply_proof: None,
    })
    .unwrap()
    .restored_code;
    assert!(restored.contains("unicodeTemplate"));
    assert!(restored.contains("hé 😀"));
}

#[test]
fn syntax_error_outside_selected_scope_is_medium_related() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("partial.py");
    std::fs::write(
        &path,
        "def good(value):\n    return value + 1\n\ndef broken(value):\n    if value >\n        return value\n",
    )
    .unwrap();
    let idx = index_path(&path).unwrap();
    let good = idx.scopes.iter().find(|s| s.name == "good").unwrap();
    assert_eq!(good.confidence, Confidence::Medium);
    assert_eq!(good.fallback_action, FallbackAction::Related);
    let exp = expand_scope(&path, "good", "symbol").unwrap();
    assert_eq!(exp.confidence, Confidence::Medium);
}

#[test]
fn first_wave_corpus_contains_hostile_required_patterns() {
    for (dir, marker) in [
        ("python", "hé 😀"),
        ("javascript", "`hé 😀"),
        ("jsx", "<div"),
        ("typescript", ": string"),
        ("tsx", "<div"),
        ("rust", "format!"),
        ("go", "func UnicodeMessage"),
    ] {
        let corpus_dir = root().join("corpus").join(dir);
        let mut saw_marker = false;
        let mut saw_comment = false;
        for entry in std::fs::read_dir(&corpus_dir)
            .unwrap()
            .filter_map(Result::ok)
        {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            saw_marker |= text.contains(marker);
            saw_comment |= text.contains("// keep") || text.contains("# keep");
        }
        assert!(saw_marker, "{dir} missing hostile marker {marker}");
        assert!(saw_comment, "{dir} missing hostile comment fixture");
    }
}

#[test]
fn line_comments_do_not_swallow_following_statement_after_minify() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("line_comment.js");
    std::fs::write(
        &path,
        "function keepLineComment(value) {\n  // keep separator\n  const text = \"a : b\";\n  return text + value;\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "keepLineComment", "symbol").unwrap();
    assert!(exp.compact_code.contains("// keep separator\n"));
    assert!(exp.compact_code.contains("\nconst") || exp.compact_code.contains("\n const"));
    assert!(!exp.compact_code.contains("// keep separator const"));
    let restored = restore_payload(RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        base_compact_code: None,
        context_ref: None,
        compact_code: Some(exp.compact_code),
        code: None,
        patch: None,
        language: Some("javascript".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map),
        apply_proof: None,
    })
    .unwrap()
    .restored_code;
    assert!(restored.contains("keepLineComment"));
    assert!(restored.contains("const"));
    assert!(restored.contains("return"));
}

#[test]
fn line_comments_preserve_terminators_and_asi_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("asi.js");
    std::fs::write(
        &path,
        "function asiBoundary() {\n  return // contains */ terminator\n  value;\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "asiBoundary", "symbol").unwrap();
    assert!(exp.compact_code.contains("// contains */ terminator\n"));
    assert!(!exp.compact_code.contains("/* contains */ terminator*/"));
    assert!(exp
        .compact_code
        .contains("return // contains */ terminator\n"));
}

#[test]
fn rust_macro_names_are_not_symbol_renamed() {
    let exp = expand_scope(
        root().join("corpus/rust/hostile_unicode_nested.rs"),
        "hostile_unicode_nested.rs:unicode_message:1",
        "symbol",
    )
    .unwrap();
    assert!(!exp.symbol_map.symbols.contains_key("format"));
    assert!(exp.compact_code.contains("format!"), "{}", exp.compact_code);
    assert!(!exp.compact_code.contains("c!"), "{}", exp.compact_code);
}

#[test]
fn rust_lifetimes_do_not_mask_following_identifiers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lifetime.rs");
    std::fs::write(
        &path,
        "pub fn borrow_value<'a>(input: &'a str) -> &'a str {\n    let value = input;\n    value\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "borrow_value", "symbol").unwrap();
    assert!(exp.symbol_map.symbols.contains_key("input"));
    assert!(exp.symbol_map.symbols.contains_key("value"));
    assert!(exp.compact_code.contains("'a"), "{}", exp.compact_code);
    let restored = restore_payload(RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        base_compact_code: None,
        context_ref: None,
        compact_code: Some(exp.compact_code),
        code: None,
        patch: None,
        language: Some("rust".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map),
        apply_proof: None,
    })
    .unwrap()
    .restored_code;
    assert!(restored.contains("borrow_value"));
    assert!(restored.contains("input"));
    assert!(restored.contains("value"));
}

#[test]
fn rust_raw_strings_and_raw_identifiers_are_not_corrupted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("raw.rs");
    std::fs::write(
        &path,
        "pub fn raw_tokens() -> &'static str {\n    let r#type = r#\"hé raw\"#;\n    r#type\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "raw_tokens", "symbol").unwrap();
    assert!(
        exp.compact_code.contains("r#\"hé raw\"#"),
        "{}",
        exp.compact_code
    );
    assert!(!exp.compact_code.contains("b#\""), "{}", exp.compact_code);
    assert!(!exp.compact_code.contains("a#type"), "{}", exp.compact_code);
    let restored = restore_payload(RestorePayload {
        scope_id: Some(exp.scope.id.clone()),
        scope: Some(exp.scope.clone()),
        compactness: Some("symbol".into()),
        base_compact_code: None,
        context_ref: None,
        compact_code: Some(exp.compact_code),
        code: None,
        patch: None,
        language: Some("rust".into()),
        symbols: None,
        reverse: None,
        symbol_map: Some(exp.symbol_map),
        apply_proof: None,
    })
    .unwrap()
    .restored_code;
    assert!(restored.contains("r#\"hé raw\"#"));
    assert!(restored.contains("r#type"));
}

#[test]
fn rust_literal_prefixes_are_not_symbol_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefix.rs");
    std::fs::write(
        &path,
        "pub fn literal_prefixes() -> &'static [u8] {\n    let raw = r\"hello\";\n    let bytes = b\"bytes\";\n    let raw_bytes = br#\"raw bytes\"#;\n    raw_bytes\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "literal_prefixes", "symbol").unwrap();
    assert!(
        exp.compact_code.contains("r\"hello\""),
        "{}",
        exp.compact_code
    );
    assert!(
        exp.compact_code.contains("b\"bytes\""),
        "{}",
        exp.compact_code
    );
    assert!(
        exp.compact_code.contains("br#\"raw bytes\"#"),
        "{}",
        exp.compact_code
    );
    assert!(
        !exp.compact_code.contains("a\"hello\""),
        "{}",
        exp.compact_code
    );
}

#[test]
fn rust_raw_strings_with_internal_quotes_do_not_rename_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("raw_quotes.rs");
    std::fs::write(
        &path,
        "pub fn raw_quotes() -> &'static str {\n    let message = r#\"quoted \"message other\" word\"#;\n    message\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "raw_quotes", "symbol").unwrap();
    assert!(
        exp.compact_code
            .contains("r#\"quoted \"message other\" word\"#"),
        "{}",
        exp.compact_code
    );
    assert!(
        !exp.compact_code.contains("r#\"quoted \"a other\" word\"#"),
        "{}",
        exp.compact_code
    );
}

#[test]
fn rust_c_string_prefixes_are_not_symbol_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c_strings.rs");
    std::fs::write(
        &path,
        "pub fn c_string_prefixes() {\n    let c = 1;\n    let cr = 2;\n    let text = c\"contents with text c\";\n    let raw = cr#\"quoted \\\"c\\\" text\"#;\n    let _sum = c + cr;\n}\n",
    )
    .unwrap();
    let exp = expand_scope(&path, "c_string_prefixes", "symbol").unwrap();
    assert!(
        exp.compact_code.contains("c\"contents with text c\""),
        "{}",
        exp.compact_code
    );
    assert!(
        exp.compact_code.contains("cr#\"quoted \\\"c\\\" text\"#"),
        "{}",
        exp.compact_code
    );
    assert!(
        !exp.compact_code.contains("d\"contents"),
        "{}",
        exp.compact_code
    );
}

#[test]
fn rust_git_checks_not_successful_is_critical() {
    let dir = tempfile::tempdir().unwrap();
    let summary = summarize_command_output(
        "gh pr checks 42",
        "Some checks were not successful\nbuild / rust cancelled\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(summary.risk, "critical");
    assert!(summary.summary.contains("not successful"));
    assert!(!summary.summary.contains("raw_ref="));
}

#[test]
fn rust_git_github_url_evidence_and_command_are_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let command = "gh api https://user:secret@github.com/repos/ium-team/tfy?access_token=secret";
    let raw = "failed artifact https://token-user:secret@github.com/ium-team/tfy/actions/runs/99?token=secret#frag\n";
    let summary = summarize_command_output(command, raw, 1, dir.path()).unwrap();
    assert_eq!(summary.risk, "critical");
    assert_eq!(
        summary.command,
        "gh api https://github.com/repos/ium-team/tfy"
    );
    assert!(summary
        .summary
        .contains("https://github.com/ium-team/tfy/actions/runs/99"));
    for secret in [
        "user:secret",
        "token-user",
        "access_token",
        "?token=secret",
        "#frag",
    ] {
        assert!(
            !summary.summary.contains(secret),
            "leaked {secret}: {}",
            summary.summary
        );
        assert!(
            !summary.command.contains(secret),
            "leaked {secret}: {}",
            summary.command
        );
    }
    let raw_output = raw_output(dir.path(), &summary.raw_ref, None, 3).unwrap();
    assert!(raw_output.contains("token-user:secret"));
    assert!(raw_output.contains("?token=secret#frag"));
}

#[test]
fn rust_git_github_malformed_url_redaction_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let command =
        "gh api https://user:secret@github.com:bad/repos/ium-team/tfy?access_token=secret";
    let raw = "failed artifact https://user:secret@github.com:bad/org/repo?token=secret\n";
    let summary = summarize_command_output(command, raw, 1, dir.path()).unwrap();
    assert_eq!(summary.risk, "critical");
    assert!(summary
        .command
        .contains("https://github.com/repos/ium-team/tfy"));
    assert!(summary.summary.contains("https://github.com/org/repo"));
    assert!(!summary.summary.contains(":bad"));
    assert!(!summary.summary.contains("user:secret"));
}

#[test]
fn rust_git_status_and_data_commands_are_conservative() {
    let dir = tempfile::tempdir().unwrap();
    let clean = summarize_command_output(
        "git status --branch --short",
        "## main...origin/main\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(clean.risk, "success");

    let dirty = summarize_command_output(
        "git status --short",
        " M docs/failed_checks.md\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(dirty.risk, "unknown");
    assert!(dirty.summary.contains(" M docs/failed_checks.md"));

    let long_dirty_raw = "On branch main\nChanges not staged for commit:\n  modified:   src/lib.rs\n  new file:   docs/notes.md\n\nUntracked files:\n  scratch.txt\n".repeat(20);
    let long_dirty =
        summarize_command_output("git status", &long_dirty_raw, 0, dir.path()).unwrap();
    assert_eq!(long_dirty.command_family, "git_status");
    assert_eq!(long_dirty.risk, "unknown");
    assert!(
        long_dirty.model_text.contains("changed_paths=40"),
        "{}",
        long_dirty.model_text
    );
    assert!(
        !long_dirty.model_text.contains("changed_paths=0"),
        "{}",
        long_dirty.model_text
    );

    let unicode_unknown =
        summarize_command_output("git status", &"🚀.txt\n".repeat(300), 0, dir.path()).unwrap();
    assert_eq!(unicode_unknown.command_family, "git_status");
    assert!(
        unicode_unknown.model_text.contains("changed_paths=unknown"),
        "{}",
        unicode_unknown.model_text
    );
    assert!(
        unicode_unknown.model_text.contains("raw_ref="),
        "{}",
        unicode_unknown.model_text
    );

    let conflict =
        summarize_command_output("git status --short", "UU src/app.rs\n", 0, dir.path()).unwrap();
    assert_eq!(conflict.risk, "critical");
    assert!(conflict.summary.contains("src/app.rs"));

    let diff = summarize_command_output(
        "git diff -- src/app.rs",
        "diff --git a/src/app.rs b/src/app.rs\n@@ -1 +1 @@\n-error = 'not found'\n+error = 'handled'\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(diff.risk, "unknown");
    assert!(diff.summary.contains("@@ -1 +1 @@"));
}

#[test]
fn public_summaries_redact_urls_for_generic_commands() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "artifact https://user:secret@github.com/org/repo?token=secret#frag\n";
    let summary = summarize_command_output("sh -c print-url", raw, 0, dir.path()).unwrap();
    assert!(matches!(summary.risk.as_str(), "success" | "unknown"));
    assert!(summary.summary.contains("https://github.com/org/repo"));
    assert!(!summary.summary.contains("user:secret"));
    assert!(!summary.summary.contains("?token=secret"));
    assert_eq!(
        raw_output(dir.path(), &summary.raw_ref, None, 1).unwrap(),
        raw
    );
}

#[test]
fn public_pass_through_redacts_secret_like_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "API_KEY=sk_test_123456789\n";
    let summary = summarize_command_output("env-check", raw, 0, dir.path()).unwrap();
    assert!(summary.summary.contains("API_KEY=[REDACTED]"));
    assert!(!summary.summary.contains("sk_test_123456789"));
    assert_eq!(
        raw_output(dir.path(), &summary.raw_ref, None, 1).unwrap(),
        raw
    );
}

#[test]
fn public_summaries_cap_unicode_on_char_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let raw = format!("A{}\n", "😀".repeat(100));
    let summary = summarize_command_output("unicode", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.risk, "unknown");
    assert!(summary.summary.contains("line capped"));
    assert!(summary.summary.contains("raw_ref="));
}

#[test]
fn run_command_enforces_summary_limit_without_losing_raw_ref() {
    let dir = tempfile::tempdir().unwrap();
    let summary = run_command(
        &["sh".into(), "-c".into(), "printf '%0500d' 0".into()],
        None,
        dir.path(),
        160,
    )
    .unwrap();
    assert!(summary.summary.len() < 240, "{}", summary.summary.len());
    assert!(summary.summary.contains(
        "
…
raw_ref="
    ));
    assert!(summary.summary.contains("raw_ref="));
    let raw = raw_output(dir.path(), &summary.raw_ref, None, 1).unwrap();
    assert!(raw.len() >= 500);
}

#[test]
fn explicit_git_github_policy_handles_wrapped_commands() {
    let dir = tempfile::tempdir().unwrap();
    let summary = summarize_command_output_with_policy(
        "sh -c gh pr checks 42",
        "Some checks were not successful\n",
        0,
        dir.path(),
        ToolPolicy::GitGithub,
    )
    .unwrap();
    assert_eq!(summary.risk, "critical");
}

#[cfg(unix)]
#[test]
fn raw_store_uses_private_unix_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let store = RawStore::new(dir.path()).unwrap();
    let rf = store.put("cmd", "raw", 0).unwrap();
    let dir_mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
    let file_mode = std::fs::metadata(dir.path().join(format!("{rf}.json")))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(file_mode, 0o600);
}

#[test]
fn evidence_excerpt_is_unicode_safe_around_error() {
    let dir = tempfile::tempdir().unwrap();
    let raw = format!("{}error{}\n", "😀".repeat(100), "😀".repeat(100));
    let summary = summarize_command_output("unicode-error", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.risk, "critical");
    assert!(summary.summary.contains("error"));
    assert!(summary.summary.contains("raw_ref="));
}

#[test]
fn tiny_summary_limit_caps_body_but_preserves_raw_ref() {
    let dir = tempfile::tempdir().unwrap();
    let summary = run_command(
        &["sh".into(), "-c".into(), "printf '%0500d' 0".into()],
        None,
        dir.path(),
        64,
    )
    .unwrap();
    assert!(summary.summary.contains(
        "
…
raw_ref="
    ));
    assert!(summary.summary.contains("raw_ref="));
    assert_eq!(
        raw_output(dir.path(), &summary.raw_ref, None, 1)
            .unwrap()
            .len(),
        500
    );
}

#[test]
fn git_diff_error_words_are_not_diagnostic_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let summary = summarize_command_output(
        "git diff -- src/app.rs",
        "diff --git a/src/app.rs b/src/app.rs\n@@ -1 +1 @@\n-error = 'not found'\n+error = 'handled'\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(summary.risk, "unknown");
    assert!(summary.summary.contains("@@ -1 +1 @@"));
    assert!(
        !summary
            .evidence
            .iter()
            .any(|line| line.contains("not found")),
        "{:?}",
        summary.evidence
    );
}

#[test]
fn generic_long_credential_url_is_redacted_before_evidence_excerpt() {
    let dir = tempfile::tempdir().unwrap();
    let raw = format!(
        "{} https://user:secret@github.com/org/repo/actions/runs/{}?token=secret#frag error\n",
        "noise".repeat(120),
        "9".repeat(80)
    );
    let summary = summarize_command_output("generic-critical", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.risk, "critical");
    assert!(summary.summary.contains("github.com"));
    for secret in ["user:secret", "token=secret", "#frag", ":secret@"] {
        assert!(
            !summary.summary.contains(secret),
            "leaked {secret}: {}",
            summary.summary
        );
        assert!(
            !summary.evidence.iter().any(|line| line.contains(secret)),
            "leaked {secret}: {:?}",
            summary.evidence
        );
    }
    assert!(raw_output(dir.path(), &summary.raw_ref, None, 1)
        .unwrap()
        .contains("user:secret"));
}

#[test]
fn generic_success_words_do_not_hide_later_failure_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let summary =
        summarize_command_output("wrapped-checks", "setup ok\ntoken expired\n", 0, dir.path())
            .unwrap();
    assert_eq!(summary.risk, "critical");
    assert!(summary.summary.contains("token expired"));

    let checks = summarize_command_output(
        "wrapped-gh",
        "Some checks were not successful\n",
        0,
        dir.path(),
    )
    .unwrap();
    assert_eq!(checks.risk, "critical");
    assert!(checks.summary.contains("not successful"));
}

#[test]
fn public_pass_through_redacts_prefixed_secret_like_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let summary = run_command(
        &[
            "sh".into(),
            "-c".into(),
            "printf 'OPENAI_API_KEY=sk_test_123456789\nDATABASE_PASSWORD=supersecret123\n'".into(),
        ],
        None,
        dir.path(),
        1_000_000,
    )
    .unwrap();
    assert_eq!(summary.rendering_kind, "pass_through");
    assert!(
        summary.model_text.contains("OPENAI_API_KEY=[REDACTED]"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("DATABASE_PASSWORD=[REDACTED]"),
        "{}",
        summary.model_text
    );
    assert!(
        !summary.model_text.contains("sk_test_123456789"),
        "{}",
        summary.model_text
    );
    assert!(
        !summary.model_text.contains("supersecret123"),
        "{}",
        summary.model_text
    );
}

#[test]
fn p0_command_family_golden_table_includes_only_deterministic_p0_families() {
    let cases = [
        ("git status --short", "git_status"),
        ("git diff --stat", "git_diff"),
        ("git log --oneline -5", "git_log"),
        ("gh pr checks 42", "gh_pr_checks"),
        ("cargo test --workspace", "cargo_test"),
        ("cargo clippy --all-targets", "cargo_clippy"),
        ("cargo build --workspace", "cargo_build"),
        ("cargo check --workspace", "cargo_check"),
        ("cargo fmt --check", "cargo_fmt_check"),
        ("cargo fmt -- --check", "cargo_fmt_check"),
        ("cargo fmt --all -- --check", "cargo_fmt_check"),
        ("pytest -q", "pytest"),
        ("python -m pytest", "pytest"),
        ("python3 -m pytest tests", "pytest"),
        ("npm test -- --runInBand", "npm_test"),
        ("pnpm test", "pnpm_test"),
        ("yarn test", "yarn_test"),
        ("go test ./...", "go_test"),
        ("mvn test", "maven_test"),
        ("./mvnw verify", "maven_test"),
        ("gradle test", "gradle_test"),
        ("./gradlew :app:test", "gradle_test"),
        ("tsc --noEmit", "tsc_check"),
        ("npx tsc --noEmit", "tsc_check"),
        ("pnpm exec tsc --noEmit", "tsc_check"),
        ("yarn tsc --noEmit", "tsc_check"),
        ("npm exec tsc -- --noEmit", "tsc_check"),
        ("npm run build", "generic"),
        ("pnpm build", "generic"),
        ("yarn build", "generic"),
        ("tsc --watch", "generic"),
        ("tsc --watch --noEmit", "generic"),
        ("npx tsc -w --noEmit", "generic"),
        ("sh -c echo tsc --noEmit", "generic"),
    ];
    for (command, family) in cases {
        assert_eq!(crate::classify_command_family(command), family, "{command}");
    }
}

#[test]
fn p0_java_test_summary_preserves_maven_failure_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = String::new();
    for i in 0..140 {
        raw.push_str(&format!("[INFO] Running com.example.Generated{i}Test\n"));
        raw.push_str("[INFO] Tests run: 1, Failures: 0, Errors: 0, Skipped: 0\n");
    }
    raw.push_str("[INFO] Running com.example.AuthServiceTest\n");
    raw.push_str("[ERROR] Tests run: 3, Failures: 1, Errors: 0, Skipped: 0, Time elapsed: 0.21 s <<< FAILURE! -- in com.example.AuthServiceTest\n");
    raw.push_str("[ERROR] com.example.AuthServiceTest.rejectsExpiredToken -- expected: <401> but was: <200>\n");
    raw.push_str(
        "[ERROR] at com.example.AuthServiceTest.rejectsExpiredToken(AuthServiceTest.java:42)\n",
    );
    raw.push_str("[ERROR] There are test failures.\n");
    let summary = summarize_command_output("mvn test", &raw, 1, dir.path()).unwrap();
    assert_eq!(summary.command_family, "maven_test");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("family=maven_test"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("failures_observed=1"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("AuthServiceTest.java:42"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("expected: <401>"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.len() < raw.len(),
        "{} >= {}",
        summary.model_text.len(),
        raw.len()
    );
}

#[test]
fn broad_dsl_command_families_classify_with_safety_metadata() {
    let cases = [
        ("npm install", "npm_install", true, "none"),
        ("ruff check .", "ruff", true, "none"),
        ("bundle install", "bundle-install", true, "none"),
        ("dotnet build", "dotnet-build", true, "none"),
        ("terraform plan", "terraform-plan", false, "possible"),
        ("aws s3 ls", "aws", false, "possible"),
    ];
    for (command, family, human_safe, interactive_risk) in cases {
        let metadata = crate::classify_command_strategy(command);
        assert_eq!(metadata.family, family, "{command}");
        assert_eq!(metadata.strategy_kind, "dsl", "{command}");
        assert_eq!(metadata.human_auto_safe, human_safe, "{command}");
        assert_eq!(metadata.interactive_risk, interactive_risk, "{command}");
    }
}

#[test]
fn broad_dsl_summary_preserves_diagnostics_and_redacts_public_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = String::new();
    for i in 0..120 {
        raw.push_str(&format!("downloaded package_{i}\n"));
    }
    raw.push_str("warning: deprecated package\n");
    raw.push_str("src/app.ts:42 error TS2322: token failed\n");
    raw.push_str("NPM_TOKEN=secretsecret123\n");
    let summary = summarize_command_output("npm install", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "npm_install");
    assert_eq!(summary.strategy_kind, "dsl");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("warning: deprecated package"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("src/app.ts:42"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("NPM_TOKEN=[REDACTED]"),
        "{}",
        summary.model_text
    );
    assert!(
        !summary.model_text.contains("secretsecret123"),
        "{}",
        summary.model_text
    );
    assert!(summary.model_text.len() < raw.len());
}

#[test]
fn built_in_filter_dsl_summarizes_df_without_bypassing_selector() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = String::from("Filesystem      Size  Used Avail Use% Mounted on\n");
    for i in 0..80 {
        raw.push_str(&format!("/dev/disk{i} 100G 95G 5G 95% /vol{i}\n"));
    }
    let summary = summarize_command_output("df -h", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "df");
    assert_eq!(summary.strategy_kind, "dsl");
    assert!(summary.human_auto_safe);
    assert_eq!(summary.interactive_risk, "none");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("family=df strategy=dsl"),
        "{}",
        summary.model_text
    );
    assert!(summary.model_text.contains("95%"), "{}", summary.model_text);
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
    assert!(summary.model_text.len() < raw.len());
}

#[test]
fn built_in_filters_have_inline_fixtures() {
    crate::tool_feedback::validate_built_in_filter_fixtures().unwrap();
}

#[test]
fn command_strategy_registry_exposes_p0_metadata_without_changing_classification() {
    let registry = crate::CommandStrategyRegistry;
    let metadata = registry.classify("cargo test --workspace");
    assert_eq!(metadata.family, "cargo_test");
    assert_eq!(metadata.strategy_kind, "rust");
    assert!(metadata.agent_safe);
    assert!(!metadata.human_auto_safe);
    assert_eq!(metadata.interactive_risk, "none");
    assert_eq!(
        metadata.claim_status,
        "implemented_p0_not_comparison_claimed"
    );

    let generic = crate::classify_command_strategy("unknown-tool --verbose");
    assert_eq!(generic.family, "generic");
    assert_eq!(generic.strategy_kind, "generic");
    assert_eq!(generic.interactive_risk, "unknown");
}

#[test]
fn command_summary_carries_strategy_metadata_through_no_negative_selector() {
    let dir = tempfile::tempdir().unwrap();
    let summary = summarize_command_output("cargo test", "ok\n", 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "cargo_test");
    assert_eq!(summary.strategy_kind, "rust");
    assert!(summary.agent_safe);
    assert!(!summary.human_auto_safe);
    assert_eq!(summary.interactive_risk, "none");
    assert_eq!(summary.rendering_kind, "pass_through");
    assert_eq!(summary.model_text, "ok\n");
}

#[test]
fn p0_command_family_classifier_and_cargo_summary_save_tokens() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        crate::classify_command_family("cargo test --workspace"),
        "cargo_test"
    );
    assert_eq!(
        crate::classify_command_family("git diff --stat"),
        "git_diff"
    );
    assert_eq!(
        crate::classify_command_family("gh pr checks 42"),
        "gh_pr_checks"
    );

    let mut raw = String::from("running 250 tests\n");
    for i in 0..250 {
        raw.push_str(&format!("test generated_case_{i} ... ok\n"));
    }
    raw.push_str("test result: ok. 250 passed; 0 failed; 0 ignored; finished in 1.23s\n");
    let summary = summarize_command_output("cargo test --workspace", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "cargo_test");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("family=cargo_test"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("tests_observed=250"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.len() < raw.len(),
        "{} >= {}",
        summary.model_text.len(),
        raw.len()
    );
}

#[test]
fn p0_test_failure_summary_preserves_actionable_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = String::new();
    for i in 0..160 {
        raw.push_str(&format!("test passing_case_{i} ... ok\n"));
    }
    raw.push_str("test auth_rejects_expired_token ... FAILED\n");
    raw.push_str("---- auth_rejects_expired_token stdout ----\n");
    raw.push_str("thread 'auth_rejects_expired_token' panicked at src/auth.rs:42:9:\n");
    raw.push_str("assertion failed: expected 401 got 200\n");
    raw.push_str("failures: auth_rejects_expired_token\n");
    raw.push_str("test result: FAILED. 160 passed; 1 failed; finished in 0.44s\n");
    let summary = summarize_command_output("cargo test auth", &raw, 101, dir.path()).unwrap();
    assert_eq!(summary.command_family, "cargo_test");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("auth_rejects_expired_token"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("src/auth.rs:42"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("expected 401 got 200"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
}

#[test]
fn p0_git_diff_summary_reports_shape_without_losing_raw() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "diff --git a/src/lib.rs b/src/lib.rs\nindex 111..222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,4 @@\n pub fn a() {}\n+pub fn b() {}\n-// old\n".repeat(80);
    let summary = summarize_command_output("git diff", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "git_diff");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("files_changed=80"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("hunks=80"),
        "{}",
        summary.model_text
    );
    let raw_back = raw_output(dir.path(), &summary.raw_ref, None, 1).unwrap();
    assert_eq!(raw_back, raw);
}

#[test]
fn p0_git_diff_stat_and_log_variants_do_not_claim_false_zero_counts() {
    let dir = tempfile::tempdir().unwrap();
    let stat_raw =
        " src/lib.rs        | 10 +++++-----\n README.md         |  2 ++\n 2 files changed, 7 insertions(+), 5 deletions(-)\n"
            .repeat(40);
    let stat_summary =
        summarize_command_output("git diff --stat", &stat_raw, 0, dir.path()).unwrap();
    assert_eq!(stat_summary.command_family, "git_diff");
    assert_eq!(stat_summary.rendering_kind, "summary");
    assert!(
        stat_summary.model_text.contains("files_changed=80"),
        "{}",
        stat_summary.model_text
    );
    assert!(
        stat_summary.model_text.contains("hunks=unknown"),
        "{}",
        stat_summary.model_text
    );
    assert!(
        stat_summary.model_text.contains("added_lines=280"),
        "{}",
        stat_summary.model_text
    );
    assert!(
        stat_summary.model_text.contains("deleted_lines=200"),
        "{}",
        stat_summary.model_text
    );
    assert!(
        !stat_summary.model_text.contains("files_changed=0"),
        "{}",
        stat_summary.model_text
    );
    assert!(
        !stat_summary
            .model_text
            .contains("added_lines=0 deleted_lines=0"),
        "{}",
        stat_summary.model_text
    );

    let log_raw = "feat: compact command output\nfix: preserve raw refs\n".repeat(80);
    let log_summary =
        summarize_command_output("git log --oneline --format=%s", &log_raw, 0, dir.path()).unwrap();
    assert_eq!(log_summary.command_family, "git_log");
    assert_eq!(log_summary.rendering_kind, "summary");
    assert!(
        log_summary.model_text.contains("commits_shown=unknown"),
        "{}",
        log_summary.model_text
    );
    assert!(
        !log_summary.model_text.contains("commits_shown=0"),
        "{}",
        log_summary.model_text
    );
}

#[test]
fn p0_gh_pr_checks_success_and_failure_risk_are_family_aware() {
    let dir = tempfile::tempdir().unwrap();
    let success_raw = "build pass 2m\ntest pass 1m\n0 failed\n".repeat(40);
    let success = summarize_command_output("gh pr checks 42", &success_raw, 0, dir.path()).unwrap();
    assert_eq!(success.command_family, "gh_pr_checks");
    assert_eq!(success.risk, "success");
    assert!(
        !success.model_text.contains("CRITICAL"),
        "{}",
        success.model_text
    );

    let failure_raw = "build pass 2m\ntest fail 1m\nlint action_required\n".repeat(20);
    let failure = summarize_command_output("gh pr checks 42", &failure_raw, 0, dir.path()).unwrap();
    assert_eq!(failure.command_family, "gh_pr_checks");
    assert_eq!(failure.risk, "critical");
    assert!(
        failure.model_text.contains("CRITICAL family=gh_pr_checks"),
        "{}",
        failure.model_text
    );
    assert!(
        failure.model_text.contains("action_required") || failure.model_text.contains("fail"),
        "{}",
        failure.model_text
    );

    let mixed_failure_raw = "build pass 2m\nunit 0 failed\nlint error 1m\n".repeat(20);
    let mixed_failure =
        summarize_command_output("gh pr checks 42", &mixed_failure_raw, 0, dir.path()).unwrap();
    assert_eq!(mixed_failure.command_family, "gh_pr_checks");
    assert_eq!(mixed_failure.risk, "critical");
    assert!(
        mixed_failure.model_text.contains("lint error"),
        "{}",
        mixed_failure.model_text
    );
}

#[test]
fn p0_cargo_fmt_check_summary_preserves_diff_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "Diff in /repo/src/lib.rs:1:\n fn main() {println!(\"hi\");}\nDiff in /repo/src/main.rs:7:\n     let value=1;\n"
        .repeat(50);
    let summary =
        summarize_command_output("cargo fmt --all -- --check", &raw, 1, dir.path()).unwrap();
    assert_eq!(summary.command_family, "cargo_fmt_check");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("family=cargo_fmt_check"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("format_diffs=100"),
        "{}",
        summary.model_text
    );
    assert!(
        !summary.model_text.contains("errors=0"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("src/lib.rs") || summary.model_text.contains("src/main.rs"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
    let raw_back = raw_output(dir.path(), &summary.raw_ref, None, 1).unwrap();
    assert_eq!(raw_back, raw);
}

#[test]
fn p0_cargo_fmt_check_diff_is_critical_even_with_masked_zero_exit() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "Diff in /repo/src/lib.rs:1:\n fn main() {println!(\"hi\");}\n".repeat(50);
    let summary = summarize_command_output("cargo fmt --check", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "cargo_fmt_check");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("format_diffs=50"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("src/lib.rs"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
}

#[test]
fn p0_tsc_check_summary_counts_diagnostics_without_false_zero() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "src/index.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.\nsrc/app.tsx(4,1): error TS2304: Cannot find name 'Widget'.\nFound 2 errors in 2 files.\n"
        .repeat(40);
    let summary = summarize_command_output("pnpm exec tsc --noEmit", &raw, 2, dir.path()).unwrap();
    assert_eq!(summary.command_family, "tsc_check");
    assert_eq!(summary.risk, "critical");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("family=tsc_check"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("diagnostics=80"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("TS2322"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("src/index.ts"),
        "{}",
        summary.model_text
    );
    assert!(
        summary.model_text.contains("raw_ref="),
        "{}",
        summary.model_text
    );
}

#[test]
fn p0_tsc_check_unknown_output_does_not_claim_zero_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "TypeScript compiler output with custom plugin notes\n".repeat(80);
    let summary = summarize_command_output("tsc --noEmit", &raw, 0, dir.path()).unwrap();
    assert_eq!(summary.command_family, "tsc_check");
    assert_eq!(summary.risk, "unknown");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(
        summary.model_text.contains("diagnostics=unknown"),
        "{}",
        summary.model_text
    );
    assert!(
        !summary.model_text.contains("diagnostics=0"),
        "{}",
        summary.model_text
    );
}

#[test]
fn suppressed_output_savings_pct_is_never_negative() {
    let dir = tempfile::tempdir().unwrap();
    let command = format!("binary-ish {}", "very-long-command-fragment".repeat(20));
    let summary = summarize_command_output(&command, "x\0", 0, dir.path()).unwrap();
    assert_eq!(summary.rendering_kind, "suppressed");
    assert_eq!(summary.savings_pct, 0.0);
    assert!(summary.model_text.len() > summary.raw_chars);
}

#[test]
fn user_toml_rule_summarizes_custom_command_and_redacts() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
[[command]]
id = "internal_build"
match.argv_prefix = ["my-build", "run"]
preserve_lines_matching = ["(?i)(error|warning|token)"]
strip_lines_matching = ["(?i)^progress"]
head_lines = 4
tail_lines = 2
max_lines = 8
truncate_lines_at = 160
on_empty = "internal_build: no relevant output"
human_auto_safe = true
agent_safe = true
interactive_risk = "none"
"#,
        "user",
    )
    .unwrap();
    let raw = format!(
        "{}ERROR: failed with NPM_TOKEN=super-secret-value\nwarning: deprecated\n",
        "progress downloading packages\n".repeat(120)
    );
    let argv = vec!["my-build".to_string(), "run".to_string()];
    let summary = summarize_command_output_with_rules(
        "my-build run",
        &argv,
        &raw,
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("internal_build"));
    assert_eq!(summary.strategy_source_kind, "user");
    assert_eq!(summary.rendering_kind, "summary");
    assert!(summary.model_text.contains("strategy=user_toml"));
    assert!(summary.model_text.contains("rule_id=internal_build"));
    assert!(!summary.model_text.contains("super-secret-value"));
    assert!(summary.model_text.contains("[REDACTED]"));
}

#[test]
fn user_toml_rule_cannot_shadow_built_in_v1() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
[[command]]
id = "shadow_df"
match.argv_prefix = ["df"]
preserve_lines_matching = ["(?i)filesystem"]
max_lines = 8
"#,
        "user",
    )
    .unwrap();
    let raw = "Filesystem      Size  Used Avail Use% Mounted on\n/dev/disk1s1    100G   95G    5G  95% /\n".repeat(40);
    let argv = vec!["df".to_string()];
    let summary =
        summarize_command_output_with_rules("df", &argv, &raw, 0, dir.path(), Some(&rules))
            .unwrap();
    assert_eq!(summary.command_family, "df");
    assert_eq!(summary.strategy_kind, "dsl");
    assert_eq!(summary.rule_id, None);
    assert!(summary
        .command_rule_diagnostics
        .iter()
        .any(|d| d.code == "user_rule_shadowed_by_builtin"));
}

#[test]
fn user_toml_strict_rejects_unknown_fields_and_invalid_regex() {
    let unknown = r#"
[[command]]
id = "bad"
match.argv_prefix = ["bad"]
shell = "echo nope"
"#;
    assert!(CommandRuleSet::from_toml_str_strict(unknown, "user").is_err());
    let invalid_regex = r#"
[[command]]
id = "bad_regex"
match.argv_prefix = ["bad"]
preserve_lines_matching = ["("]
"#;
    assert!(CommandRuleSet::from_toml_str_strict(invalid_regex, "user").is_err());
}

#[test]
fn user_toml_long_summary_keeps_no_negative_passthrough() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
[[command]]
id = "tiny"
match.argv_prefix = ["tiny"]
preserve_lines_matching = ["ok"]
head_lines = 16
tail_lines = 8
max_lines = 32
"#,
        "user",
    )
    .unwrap();
    let argv = vec!["tiny".to_string()];
    let summary =
        summarize_command_output_with_rules("tiny", &argv, "ok", 0, dir.path(), Some(&rules))
            .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rendering_kind, "pass_through");
    assert_eq!(summary.model_text, "ok");
    assert_eq!(summary.savings_pct, 0.0);
}

#[test]
fn user_toml_non_strict_keeps_valid_rules_when_one_rule_is_bad() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    let rules = r#"
[[command]]
id = "valid"
match.argv_prefix = ["custom"]
preserve_lines_matching = ["ERROR"]
max_lines = 8

[[command]]
id = "bad_regex"
match.argv_prefix = ["bad"]
preserve_lines_matching = ["("]
"#;
    std::fs::write(tfy.join("commands.toml"), rules).unwrap();
    let hash = format!("{:x}", Sha256::digest(rules.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();

    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_invalid_regex"));
    let argv = vec!["custom".to_string()];
    let summary = summarize_command_output_with_rules(
        "custom",
        &argv,
        &("noise\n".repeat(120) + "ERROR kept\n"),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("valid"));
}

#[test]
fn user_toml_non_strict_reports_unknown_top_level_keys_with_valid_rules() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    let rules = r#"
unknown = "typo"

[[command]]
id = "valid"
match.argv_prefix = ["custom"]
preserve_lines_matching = ["ERROR"]
max_lines = 8
"#;
    std::fs::write(tfy.join("commands.toml"), rules).unwrap();
    let hash = format!("{:x}", Sha256::digest(rules.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();

    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_unsupported_field"));
    let argv = vec!["custom".to_string()];
    let summary = summarize_command_output_with_rules(
        "custom",
        &argv,
        &("noise\n".repeat(120) + "ERROR kept\n"),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("valid"));
}

#[test]
fn user_toml_on_empty_does_not_leak_rule_path() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
[[command]]
id = "empty_rule"
match.argv_prefix = ["empty"]
keep_lines_matching = ["NEVER_MATCHES"]
max_lines = 8
"#,
        "user",
    )
    .unwrap();
    let argv = vec!["empty".to_string()];
    let summary = summarize_command_output_with_rules(
        "empty",
        &argv,
        &"noise\n".repeat(120),
        0,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert!(summary
        .model_text
        .contains("empty_rule: no relevant output"));
    assert!(!summary.model_text.contains("<memory>"));
    assert!(!summary
        .model_text
        .contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn command_rule_diagnostic_deserializes_from_partial_object() {
    let diagnostic: CommandRuleDiagnostic =
        serde_json::from_str(r#"{"code":"repo_rules_untrusted"}"#).unwrap();
    assert_eq!(diagnostic.code, "repo_rules_untrusted");
    assert!(diagnostic.source_kind.is_empty());
    assert!(diagnostic.path.is_empty());
    assert!(diagnostic.message.is_empty());
}

#[test]
fn user_toml_v2_sections_counters_captures_and_severity_share_safety_gate() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 2

[[command]]
id = "project_build"
match.argv_prefix = ["project-build"]
strip_lines_matching = ["(?i)^progress"]
head_lines = 4
tail_lines = 2
max_lines = 8
truncate_lines_at = 180

[[command.section]]
name = "errors"
title = "Errors"
keep_lines_matching = ["(?i)(error|failed|fatal)"]
max_lines = 10
truncate_lines_at = 180

[[command.counter]]
name = "errors"
match = "(?i)(error|failed|fatal)"

[[command.capture]]
name = "files"
pattern = "(?m)^(?<file>[^:\\s][^:]+):(?<line>\\d+):"
field = "file"
dedupe = true
max_items = 5

[[command.severity]]
level = "critical"
match = "(?i)fatal|panic|failed"
"#,
        "user",
    )
    .unwrap();
    let raw = format!(
        "{}src/app.ts:10: ERROR failed NPM_TOKEN=super-secret-value\nsrc/app.ts:11: warning ignored\nsrc/lib.ts:22: fatal https://user:password@example.com\n",
        "progress compiling\n".repeat(160)
    );
    let argv = vec!["project-build".to_string()];
    let summary = summarize_command_output_with_rules(
        "project-build",
        &argv,
        &raw,
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("project_build"));
    assert_eq!(summary.rendering_kind, "summary");
    assert!(summary.model_text.contains("custom_severity=critical"));
    assert!(summary.model_text.contains("counter.errors="));
    assert!(summary.model_text.contains("Errors:"));
    assert!(summary.model_text.contains("files:"));
    assert!(summary.model_text.contains("src/app.ts"));
    assert!(summary.model_text.contains("src/lib.ts"));
    assert!(!summary.model_text.contains("super-secret-value"));
    assert!(!summary.model_text.contains("user:password"));
    assert!(summary.model_text.contains("[REDACTED]"));
    assert!(summary.model_text.contains("raw_ref="));
}

#[test]
fn user_toml_v2_tiny_output_still_uses_no_negative_passthrough() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 2

[[command]]
id = "tiny_v2"
match.argv_prefix = ["tiny-v2"]

[[command.counter]]
name = "ok"
match = "ok"
"#,
        "user",
    )
    .unwrap();
    let argv = vec!["tiny-v2".to_string()];
    let summary =
        summarize_command_output_with_rules("tiny-v2", &argv, "ok", 0, dir.path(), Some(&rules))
            .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rendering_kind, "pass_through");
    assert_eq!(summary.model_text, "ok");
    assert_eq!(summary.savings_pct, 0.0);
}

#[test]
fn user_toml_v2_rejects_invalid_capture_and_schema_version() {
    let bad_capture = r#"
schema_version = 2

[[command]]
id = "bad_capture"
match.argv_prefix = ["bad"]

[[command.capture]]
name = "files"
pattern = "(?<other>.+)"
field = "file"
"#;
    assert!(CommandRuleSet::from_toml_str_strict(bad_capture, "user").is_err());
    let bad_version = r#"
schema_version = 999

[[command]]
id = "bad_version"
match.argv_prefix = ["bad"]
"#;
    assert!(CommandRuleSet::from_toml_str_strict(bad_version, "user").is_err());
}

#[test]
fn user_toml_v2_operations_require_schema_version_two() {
    let missing_version = r#"
[[command]]
id = "missing_v2"
match.argv_prefix = ["missing-v2"]

[[command.section]]
name = "errors"
keep_lines_matching = ["ERROR"]
"#;
    assert!(CommandRuleSet::from_toml_str_strict(missing_version, "user").is_err());

    let explicit_v1 = r#"
schema_version = 1

[[command]]
id = "explicit_v1"
match.argv_prefix = ["explicit-v1"]

[[command.counter]]
name = "errors"
match = "ERROR"
"#;
    assert!(CommandRuleSet::from_toml_str_strict(explicit_v1, "user").is_err());
}

#[test]
fn user_toml_non_strict_rejects_invalid_schema_type_and_v1_v2_mixture() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    let invalid_schema_type = r#"
schema_version = "2"

[[command]]
id = "bad_schema_type"
match.argv_prefix = ["bad-schema-type"]
"#;
    std::fs::write(tfy.join("commands.toml"), invalid_schema_type).unwrap();
    let hash = format!("{:x}", Sha256::digest(invalid_schema_type.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();
    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_unsupported_schema_version"));
    let argv = vec!["bad-schema-type".to_string()];
    let summary = summarize_command_output_with_rules(
        "bad-schema-type",
        &argv,
        &("noise\n".repeat(120) + "ERROR kept\n"),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_ne!(summary.strategy_kind, "user_toml");

    let mixed_v1 = r#"
[[command]]
id = "valid_v1"
match.argv_prefix = ["valid-v1"]
keep_lines_matching = ["KEEP"]

[[command]]
id = "bad_v2_without_version"
match.argv_prefix = ["bad-v2-without-version"]

[[command.severity]]
level = "error"
match = "ERROR"
"#;
    std::fs::write(tfy.join("commands.toml"), mixed_v1).unwrap();
    let hash = format!("{:x}", Sha256::digest(mixed_v1.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();
    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_unsupported_schema_version"));
    let argv = vec!["valid-v1".to_string()];
    let summary = summarize_command_output_with_rules(
        "valid-v1",
        &argv,
        &("noise\n".repeat(120) + "KEEP this line\n"),
        0,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("valid_v1"));
}

#[test]
fn user_toml_v2_non_strict_reports_invalid_operation_and_keeps_valid_rule() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    let rules = r#"
schema_version = 2

[[command]]
id = "valid_v2"
match.argv_prefix = ["valid-v2"]

[[command.counter]]
name = "errors"
match = "ERROR"

[[command]]
id = "bad_v2"
match.argv_prefix = ["bad-v2"]

[[command.capture]]
name = "files"
pattern = "(?<other>.+)"
field = "file"
"#;
    std::fs::write(tfy.join("commands.toml"), rules).unwrap();
    let hash = format!("{:x}", Sha256::digest(rules.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();
    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_invalid_capture"));
    let argv = vec!["valid-v2".to_string()];
    let summary = summarize_command_output_with_rules(
        "valid-v2",
        &argv,
        &("noise\n".repeat(120) + "ERROR kept\n"),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("valid_v2"));
}

#[test]
fn user_toml_v3_structured_metrics_and_groups_share_safety_gate() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 3

[[command]]
id = "quality_report"
match.argv_prefix = ["quality-report"]
strip_lines_matching = ["^progress"]
max_lines = 12
truncate_lines_at = 180

[[command.parse_ndjson]]
name = "json_files"
path = "errors[*].file"
max_items = 4

[[command.parse_ndjson]]
name = "ndjson_levels"
path = "level"
max_items = 3

[[command.parse_kv]]
name = "duration"
key = "duration_ms"
separators = ["="]
max_items = 2

[[command.parse_table]]
name = "table_failures"
columns = ["file", "status"]
delimiter = "whitespace"
max_rows = 3

[[command.metric]]
name = "error_mentions"
op = "count"
match = "ERROR"
max_count = 99

[[command.group]]
name = "by_file"
pattern = "file=(?<file>[^\\s]+)"
field = "file"
top_k = 3

[[command.severity]]
level = "critical"
match = "ERROR|failed"
"#,
        "user",
    )
    .unwrap();
    let raw = format!(
        "{}\n{}\n{}\n{}{}",
        r#"{"errors":[{"file":"src/app.ts","token":"NPM_TOKEN=super-secret-value"},{"file":"src/lib.ts"}]}"#,
        r#"{"level":"ERROR","message":"failed","secret":"ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        "duration_ms=1234",
        "file status\nsrc/app.ts failed\nsrc/lib.ts ok\n",
        "file=src/app.ts ERROR\nfile=src/app.ts ERROR\nfile=src/lib.ts ERROR\n",
    );
    let argv = vec!["quality-report".to_string()];
    let summary = summarize_command_output_with_rules(
        "quality-report",
        &argv,
        &raw.repeat(40),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("quality_report"));
    assert_eq!(summary.rendering_kind, "summary");
    assert!(summary.model_text.contains("custom_severity=critical"));
    assert!(summary.model_text.contains("counter.error_mentions="));
    assert!(summary.model_text.contains("json_files:"));
    assert!(summary.model_text.contains("src/app.ts"));
    assert!(summary.model_text.contains("ndjson_levels:"));
    assert!(summary.model_text.contains("duration:"));
    assert!(summary.model_text.contains("table_failures:"));
    assert!(summary.model_text.contains("file=src/app.ts status=failed"));
    assert!(summary.model_text.contains("group.by_file:"));
    assert!(!summary.model_text.contains("super-secret-value"));
    assert!(!summary.model_text.contains("ghp_aaaaaaaa"));
    assert!(summary.model_text.contains("[REDACTED]"));
    assert!(summary.model_text.contains("raw_ref="));
}

#[test]
fn user_toml_v3_operations_require_schema_version_three() {
    let missing_version = r#"
[[command]]
id = "missing_v3"
match.argv_prefix = ["missing-v3"]

[[command.parse_json]]
name = "files"
path = "files[*]"
"#;
    assert!(CommandRuleSet::from_toml_str_strict(missing_version, "user").is_err());

    let explicit_v2 = r#"
schema_version = 2

[[command]]
id = "explicit_v2"
match.argv_prefix = ["explicit-v2"]

[[command.metric]]
name = "errors"
op = "count"
match = "ERROR"
"#;
    assert!(CommandRuleSet::from_toml_str_strict(explicit_v2, "user").is_err());
}

#[test]
fn user_toml_v3_non_strict_reports_invalid_operation_and_keeps_valid_rule() {
    let dir = tempfile::tempdir().unwrap();
    let tfy = dir.path().join(".tfy");
    std::fs::create_dir_all(&tfy).unwrap();
    let rules = r#"
schema_version = 3

[[command]]
id = "valid_v3"
match.argv_prefix = ["valid-v3"]

[[command.metric]]
name = "errors"
op = "count"
match = "ERROR"

[[command]]
id = "bad_group"
match.argv_prefix = ["bad-group"]

[[command.group]]
name = "by_file"
pattern = "(?<other>.+)"
field = "file"
"#;
    std::fs::write(tfy.join("commands.toml"), rules).unwrap();
    let hash = format!("{:x}", Sha256::digest(rules.as_bytes()));
    std::fs::write(
        tfy.join("trust.json"),
        serde_json::json!({
            "schema_version": 1,
            "command_rules": {"trusted": true, "rules_sha256": hash}
        })
        .to_string(),
    )
    .unwrap();
    let rules = CommandRuleSet::load_standard(dir.path());
    assert!(rules
        .diagnostics()
        .iter()
        .any(|d| d.code == "user_rules_invalid_group"));
    let argv = vec!["valid-v3".to_string()];
    let summary = summarize_command_output_with_rules(
        "valid-v3",
        &argv,
        &("noise\n".repeat(120) + "ERROR kept\n"),
        1,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert_eq!(summary.rule_id.as_deref(), Some("valid_v3"));
}

#[test]
fn user_toml_v3_still_cannot_shadow_built_in_without_override_control_plane() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 3

[[command]]
id = "shadow_df_v3"
match.argv_prefix = ["df"]

[[command.metric]]
name = "filesystems"
op = "count"
match = "Filesystem"
"#,
        "user",
    )
    .unwrap();
    let raw = "Filesystem      Size  Used Avail Use% Mounted on\n/dev/disk1s1    100G   95G    5G  95% /\n".repeat(40);
    let argv = vec!["df".to_string()];
    let summary =
        summarize_command_output_with_rules("df", &argv, &raw, 0, dir.path(), Some(&rules))
            .unwrap();
    assert_eq!(summary.command_family, "df");
    assert_eq!(summary.strategy_kind, "dsl");
    assert_eq!(summary.rule_id, None);
    assert!(summary
        .command_rule_diagnostics
        .iter()
        .any(|d| d.code == "user_rule_shadowed_by_builtin"));
}

#[test]
fn user_toml_v3_group_distinct_values_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 3

[[command]]
id = "many_groups"
match.argv_prefix = ["many-groups"]
keep_lines_matching = ["NEVER_MATCHES"]

[[command.group]]
name = "by_id"
pattern = 'id=(?<id>[^\s]+)'
field = "id"
top_k = 10
"#,
        "user",
    )
    .unwrap();
    let raw = (0..150)
        .map(|idx| format!("id=item-{idx}\n"))
        .collect::<String>();
    let argv = vec!["many-groups".to_string()];
    let summary = summarize_command_output_with_rules(
        "many-groups",
        &argv,
        &raw,
        0,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert!(summary.model_text.contains("group.by_id:"));
    assert!(summary
        .model_text
        .contains("[truncated distinct groups at 100]"));
    assert!(!summary.model_text.contains("item-149"));
}

#[test]
fn user_toml_v3_table_requires_all_requested_columns() {
    let dir = tempfile::tempdir().unwrap();
    let rules = CommandRuleSet::from_toml_str_strict(
        r#"
schema_version = 3

[[command]]
id = "partial_table"
match.argv_prefix = ["partial-table"]
max_lines = 8

[[command.parse_table]]
name = "table"
columns = ["file", "status"]
delimiter = "whitespace"
max_rows = 5
"#,
        "user",
    )
    .unwrap();
    let raw = "file owner\nsrc/app.ts alice\n".repeat(40);
    let argv = vec!["partial-table".to_string()];
    let summary = summarize_command_output_with_rules(
        "partial-table",
        &argv,
        &raw,
        0,
        dir.path(),
        Some(&rules),
    )
    .unwrap();
    assert_eq!(summary.strategy_kind, "user_toml");
    assert!(
        !summary.model_text.contains("table:"),
        "{}",
        summary.model_text
    );
}
