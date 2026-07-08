use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::Command;

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn proof(id: &str, base: Option<&str>, preview: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "proof_id": id,
        "source_ref": format!("ctx-{id}"),
        "validation_status": "valid",
        "authority": "workspace_apply",
        "base_file_hash": base,
        "restored_preview_hash": preview
    })
}

#[test]
fn agent_run_records_agent_origin_without_human_shell_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let raw = dir.path().join("raw");

    let install = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "agent",
            "install",
            "--dry-run",
            "--output",
            "tfy-agent-wrapper",
        ])
        .output()
        .unwrap();
    assert!(install.status.success());
    let install_text = String::from_utf8_lossy(&install.stdout);
    assert!(
        install_text.contains("ordinary_terminal_interception=false"),
        "{install_text}"
    );
    assert!(
        install_text.contains("mutates_shell_startup_files_by_default=false"),
        "{install_text}"
    );
    assert!(!dir.path().join(".zshrc").exists());
    assert!(!dir.path().join("tfy-agent-wrapper").exists());

    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "agent",
            "run",
            "--session",
            "agent-s",
            "--ledger",
            ledger.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
            "--json",
            "--",
            "sh",
            "-c",
            "printf agent-ok",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["origin"]["kind"], "agent_runtime");
    assert_eq!(json["origin"]["invocation"], "wrapper");
    assert_eq!(json["origin"]["intercepted"], true);
    assert_eq!(json["origin"]["user_shell_mutated"], false);

    let ledger_text = std::fs::read_to_string(&ledger).unwrap();
    let event: serde_json::Value =
        serde_json::from_str(ledger_text.lines().next().unwrap()).unwrap();
    assert_eq!(event["origin"]["kind"], "agent_runtime");

    let before = std::fs::read_to_string(&ledger).unwrap();
    let human = Command::new("sh")
        .current_dir(dir.path())
        .args(["-c", "printf human-ok"])
        .output()
        .unwrap();
    assert!(human.status.success());
    assert_eq!(std::fs::read_to_string(&ledger).unwrap(), before);
}

#[test]
fn agent_run_records_custom_guidance_without_stdout_pollution_and_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "agent",
            "run",
            "--session",
            "agent-guidance",
            "--ledger",
            ".tfy/agent/ledger.jsonl",
            "--raw-dir",
            ".tfy/raw",
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo custom-line-$i; done",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TFY command summary"), "{stdout}");
    assert!(!stdout.contains("tfy custom"), "{stdout}");
    assert!(!stdout.contains("custom-guidance"), "{stdout}");

    let guidance_path = dir.path().join(".tfy/agent/custom-guidance.jsonl");
    let guidance = std::fs::read_to_string(&guidance_path).unwrap();
    assert!(guidance.contains("agent-guidance"), "{guidance}");
    assert!(
        guidance.contains("generic_summary_without_trusted_builtin_or_custom_rule"),
        "{guidance}"
    );

    let report = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(["agent", "report", "--session", "agent-guidance"])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    let report_stdout = String::from_utf8_lossy(&report.stdout);
    assert!(
        report_stdout.contains("needs_custom_rules=true"),
        "{report_stdout}"
    );
    assert!(
        report_stdout.contains("custom_rule_candidate command=sh"),
        "{report_stdout}"
    );
}

#[test]
fn agent_run_does_not_record_custom_guidance_for_non_summary_renderings() {
    let suppressed_dir = tempfile::tempdir().unwrap();
    let suppressed = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(suppressed_dir.path())
        .args([
            "agent",
            "run",
            "--session",
            "agent-suppressed-guidance",
            "--ledger",
            ".tfy/agent/ledger.jsonl",
            "--raw-dir",
            ".tfy/raw",
            "--",
            "python3",
            "-c",
            "import sys; sys.stdout.buffer.write(b'abc\\x00def')",
        ])
        .output()
        .unwrap();
    assert!(
        suppressed.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&suppressed.stderr)
    );
    let suppressed_stdout = String::from_utf8_lossy(&suppressed.stdout);
    assert!(
        suppressed_stdout.contains("output suppressed"),
        "{suppressed_stdout}"
    );
    assert!(
        !suppressed_dir
            .path()
            .join(".tfy/agent/custom-guidance.jsonl")
            .exists(),
        "suppressed output is not a generic summary candidate"
    );

    let repeat_dir = tempfile::tempdir().unwrap();
    let repeated_args = [
        "agent",
        "run",
        "--session",
        "agent-repeat-guidance",
        "--ledger",
        ".tfy/agent/ledger.jsonl",
        "--raw-dir",
        ".tfy/raw",
        "--",
        "sh",
        "-c",
        "for i in $(seq 1 80); do echo repeat-line-$i; done",
    ];
    let first = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(repeat_dir.path())
        .args(repeated_args)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    let guidance_path = repeat_dir.path().join(".tfy/agent/custom-guidance.jsonl");
    assert!(guidance_path.exists());
    std::fs::remove_file(&guidance_path).unwrap();

    let second = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(repeat_dir.path())
        .args(repeated_args)
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let repeat_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        repeat_stdout.contains("repeated unchanged command output elided"),
        "{repeat_stdout}"
    );
    assert!(
        !guidance_path.exists(),
        "repeat elision is not a generic summary candidate"
    );
}

#[test]
fn restore_display_is_readable_and_not_apply_authority() {
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id":"s",
            "language":"javascript",
            "compactness":"symbol",
            "compact_code":"function f0(a,b){{const c=a+b;return c;}}",
            "symbols":{"calculateTotal":"f0","left":"a","right":"b","total":"c"}
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "restore-display",
            "--payload",
            payload.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let display = json["display_code"].as_str().unwrap();
    assert!(display.contains("calculateTotal"), "{display}");
    assert!(display.contains("\n"), "{display}");
    assert_eq!(json["display_only"], true);
    assert_eq!(json["authority"], "display_only_not_apply_authority");
}

#[test]
fn workspace_validate_and_apply_exact_plan_with_hash_proof() {
    let dir = tempfile::tempdir().unwrap();
    let old = "alpha
";
    std::fs::write(dir.path().join("a.txt"), old).unwrap();
    let plan_path = dir.path().join("plan.json");
    let mut plan = serde_json::json!({
        "plan_id":"p1",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[
            {"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex(old),"content":"beta
","restored_preview_hash":sha256_hex("beta
"),"full_file_scope":true,"per_op_proof":proof("proof-m1", Some(&sha256_hex(old)), Some(&sha256_hex("beta
")))},
            {"op_id":"a1","kind":"add","path_after":"b.txt","content":"new
","restored_preview_hash":sha256_hex("new
"),"full_file_scope":true,"per_op_proof":proof("proof-a1", None, Some(&sha256_hex("new
")))}
        ]
    });
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();

    let validate = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&validate.stdout).unwrap();
    assert_eq!(report["status"], "valid");
    assert_eq!(report["mutated"], false);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        old
    );
    let plan_hash = report["plan_hash"].as_str().unwrap().to_string();
    plan["validation_status"] = serde_json::json!("valid");
    plan["validation_proof"] = serde_json::json!(plan_hash.clone());
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();

    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "apply",
            "--payload",
            plan_path.to_str().unwrap(),
            "--plan-hash",
            &plan_hash,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let applied: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(applied["status"], "applied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "beta
"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "new
"
    );
}

#[test]
fn workspace_rejects_stale_hash_empty_delete_and_changed_plan_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.txt"),
        "alpha
",
    )
    .unwrap();
    let plan_path = dir.path().join("bad.json");
    std::fs::write(
        &plan_path,
        serde_json::to_string(&serde_json::json!({
            "plan_id":"bad",
            "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
            "operations":[{"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("alpha
"),"content":"","full_file_scope":true,"per_op_proof":proof("proof", Some(&sha256_hex("alpha
")), None)}]
        })).unwrap(),
    ).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("empty content is not delete"));

    std::fs::write(&plan_path, serde_json::to_string(&serde_json::json!({
        "plan_id":"p2",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "validation_status":"valid",
        "validation_proof":"tfy_wrong",
        "operations":[{"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("alpha
"),"content":"beta
","restored_preview_hash":sha256_hex("beta
"),"full_file_scope":true,"per_op_proof":proof("proof", Some(&sha256_hex("alpha
")), Some(&sha256_hex("beta
")))}]
    })).unwrap()).unwrap();
    let wrong = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "apply",
            "--payload",
            plan_path.to_str().unwrap(),
            "--plan-hash",
            "tfy_wrong2",
        ])
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("validation_proof matching plan_hash"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "alpha
"
    );
}

#[test]
fn workspace_rejects_human_origin_string_proof_and_symlink_escape() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.txt"),
        "alpha
",
    )
    .unwrap();

    let string_proof = dir.path().join("string-proof.json");
    std::fs::write(
        &string_proof,
        serde_json::to_string(&serde_json::json!({
            "plan_id":"string-proof",
            "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
            "operations":[{"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("alpha
"),"content":"beta
","full_file_scope":true,"per_op_proof":"not-structured"}]
        })).unwrap(),
    ).unwrap();
    let bad_proof = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            string_proof.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad_proof.status.success());

    let human_origin = dir.path().join("human-origin.json");
    std::fs::write(
        &human_origin,
        serde_json::to_string(&serde_json::json!({
            "plan_id":"human-origin",
            "operations":[{"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("alpha
"),"content":"beta
","restored_preview_hash":sha256_hex("beta
"),"full_file_scope":true,"per_op_proof":proof("proof", Some(&sha256_hex("alpha
")), Some(&sha256_hex("beta
")))}]
        })).unwrap(),
    ).unwrap();
    let bad_origin = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            human_origin.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad_origin.status.success());
    assert!(
        String::from_utf8_lossy(&bad_origin.stderr).contains("requires explicit agent/test origin")
    );

    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        outside.path(),
        "outside
",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link.txt")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(outside.path(), dir.path().join("link.txt")).unwrap();
    let symlink_plan = dir.path().join("symlink.json");
    std::fs::write(
        &symlink_plan,
        serde_json::to_string(&serde_json::json!({
            "plan_id":"symlink",
            "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
            "operations":[{"op_id":"m1","kind":"modify_exact","path_before":"link.txt","base_file_hash":sha256_hex("outside
"),"content":"owned
","restored_preview_hash":sha256_hex("owned
"),"full_file_scope":true,"per_op_proof":proof("proof", Some(&sha256_hex("outside
")), Some(&sha256_hex("owned
")))}]
        })).unwrap(),
    ).unwrap();
    let bad_symlink = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            symlink_plan.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad_symlink.status.success());
    assert_eq!(
        std::fs::read_to_string(outside.path()).unwrap(),
        "outside
"
    );
}

#[test]
fn workspace_exact_delete_rename_move_are_gated() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("delete.txt"),
        "delete-me
",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("rename.txt"),
        "rename-me
",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("move.txt"),
        "move-me
",
    )
    .unwrap();
    let plan_path = dir.path().join("ops.json");
    let mut plan = serde_json::json!({
        "plan_id":"ops",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[
            {"op_id":"d1","kind":"delete","path_before":"delete.txt","base_file_hash":sha256_hex("delete-me
"),"full_file_scope":true,"per_op_proof":proof("proof-d", Some(&sha256_hex("delete-me
")), None)},
            {"op_id":"r1","kind":"rename","path_before":"rename.txt","path_after":"renamed.txt","base_file_hash":sha256_hex("rename-me
"),"full_file_scope":true,"per_op_proof":proof("proof-r", Some(&sha256_hex("rename-me
")), None)},
            {"op_id":"mv1","kind":"move","path_before":"move.txt","path_after":"sub/move.txt","base_file_hash":sha256_hex("move-me
"),"full_file_scope":true,"per_op_proof":proof("proof-mv", Some(&sha256_hex("move-me
")), None)}
        ]
    });
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let validate = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&validate.stdout).unwrap();
    let plan_hash = report["plan_hash"].as_str().unwrap().to_string();
    plan["validation_status"] = serde_json::json!("valid");
    plan["validation_proof"] = serde_json::json!(plan_hash.clone());
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "apply",
            "--payload",
            plan_path.to_str().unwrap(),
            "--plan-hash",
            &plan_hash,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(!dir.path().join("delete.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("renamed.txt")).unwrap(),
        "rename-me
"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("sub/move.txt")).unwrap(),
        "move-me
"
    );
}

#[test]
fn restore_file_writes_readable_canonical_code_not_compact_names() {
    let dir = tempfile::tempdir().unwrap();
    let payload_path = dir.path().join("restore.json");
    let output_path = dir.path().join("out.js");
    std::fs::write(
        &payload_path,
        serde_json::to_string(&serde_json::json!({
            "scope_id":"scope-calc",
            "language":"javascript",
            "compactness":"symbol",
            "compact_code":"function f0(a,b){const c=a+b;return c;}",
            "symbols":{"calculateTotal":"f0","left":"a","right":"b","total":"c"}
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "restore-file",
            "--payload",
            payload_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["canonical_for"], "file_write_and_user_display");
    assert_eq!(json["compact_transport_only"], true);
    let file = std::fs::read_to_string(&output_path).unwrap();
    assert!(file.contains("calculateTotal"), "{file}");
    assert!(file.contains("left"), "{file}");
    assert!(file.contains("right"), "{file}");
    assert!(file.contains("total"), "{file}");
    assert!(!file.contains("f0"), "{file}");
    assert!(file.contains('\n'), "{file}");
}

#[test]
fn workspace_apply_restores_compact_payload_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let old = "function calculateTotal(left, right) {\n  return left + right;\n}\n";
    std::fs::write(dir.path().join("calc.js"), old).unwrap();
    let expected = "function calculateTotal(\n  left,\n  right\n){\n  const total=left+right;\n  return total;\n}\n";
    let plan_path = dir.path().join("restore-plan.json");
    let mut plan = serde_json::json!({
        "plan_id":"restore-plan",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[{
            "op_id":"m1",
            "kind":"modify_exact",
            "path_before":"calc.js",
            "base_file_hash":sha256_hex(old),
            "restore_payload":{
                "scope_id":"scope-calc",
                "language":"javascript",
                "compactness":"symbol",
                "compact_code":"function f0(a,b){const c=a+b;return c;}",
                "symbols":{"calculateTotal":"f0","left":"a","right":"b","total":"c"}
            },
            "restored_preview_hash":sha256_hex(expected),
            "full_file_scope":true,
            "per_op_proof":proof("proof-restore", Some(&sha256_hex(old)), Some(&sha256_hex(expected)))
        }]
    });
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let validate = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&validate.stdout).unwrap();
    let plan_hash = report["plan_hash"].as_str().unwrap().to_string();
    plan["validation_status"] = serde_json::json!("valid");
    plan["validation_proof"] = serde_json::json!(plan_hash.clone());
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "apply",
            "--payload",
            plan_path.to_str().unwrap(),
            "--plan-hash",
            &plan_hash,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("calc.js")).unwrap(),
        expected
    );
}

#[test]
fn workspace_fuzzy_apply_unique_anchor_and_rejects_ambiguous() {
    let dir = tempfile::tempdir().unwrap();
    let old = "alpha\nneedle\nomega\n";
    let new = "alpha\nreplacement\nomega\n";
    std::fs::write(dir.path().join("a.txt"), old).unwrap();
    let plan_path = dir.path().join("fuzzy.json");
    let mut plan = serde_json::json!({
        "plan_id":"fuzzy",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "policy":{"allow_fuzzy_apply":true,"fuzzy_confidence_threshold":0.95},
        "operations":[{
            "op_id":"f1","kind":"modify_fuzzy","path_before":"a.txt","base_file_hash":sha256_hex(old),"anchor_before":"needle","replacement":"replacement","expected_occurrences":1,"confidence":0.99,
            "restored_preview_hash":sha256_hex(new),"full_file_scope":true,
            "per_op_proof":proof("proof-fuzzy", Some(&sha256_hex(old)), Some(&sha256_hex(new)))
        }]
    });
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let validate = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&validate.stdout).unwrap();
    let plan_hash = report["plan_hash"].as_str().unwrap().to_string();
    plan["validation_status"] = serde_json::json!("valid");
    plan["validation_proof"] = serde_json::json!(plan_hash.clone());
    std::fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "apply",
            "--payload",
            plan_path.to_str().unwrap(),
            "--plan-hash",
            &plan_hash,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        new
    );

    std::fs::write(dir.path().join("ambiguous.txt"), "needle\nneedle\n").unwrap();
    let ambiguous = dir.path().join("ambiguous.json");
    std::fs::write(&ambiguous, serde_json::to_string(&serde_json::json!({
        "plan_id":"ambiguous",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "policy":{"allow_fuzzy_apply":true},
        "operations":[{"op_id":"f2","kind":"modify_fuzzy","path_before":"ambiguous.txt","base_file_hash":sha256_hex("needle\nneedle\n"),"anchor_before":"needle","replacement":"x","expected_occurrences":1,"confidence":0.99,"restored_preview_hash":sha256_hex("x\nneedle\n"),"full_file_scope":true,"per_op_proof":proof("proof-fuzzy2", Some(&sha256_hex("needle\nneedle\n")), Some(&sha256_hex("x\nneedle\n")))}]
    })).unwrap()).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            ambiguous.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("ambiguous anchor"));
}

#[test]
fn workspace_conflict_overlap_rejected_and_refactor_plan_chunks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "abcdef\n").unwrap();
    let conflict_path = dir.path().join("conflict.json");
    std::fs::write(&conflict_path, serde_json::to_string(&serde_json::json!({
        "plan_id":"conflict",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[
            {"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("abcdef\n"),"content":"one\n","selected_range":{"byte_start":0,"byte_end":3},"per_op_proof":proof("p1", Some(&sha256_hex("abcdef\n")), None)},
            {"op_id":"m2","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex("abcdef\n"),"content":"two\n","selected_range":{"byte_start":2,"byte_end":5},"per_op_proof":proof("p2", Some(&sha256_hex("abcdef\n")), None)}
        ]
    })).unwrap()).unwrap();
    let conflict = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            conflict_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("both mutate"));

    let refactor_path = dir.path().join("refactor.json");
    std::fs::write(&refactor_path, serde_json::to_string(&serde_json::json!({
        "plan_id":"refactor",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[
            {"op_id":"a1","kind":"add","path_after":"a1.txt","content":"1\n","restored_preview_hash":sha256_hex("1\n"),"full_file_scope":true,"per_op_proof":proof("pa1", None, Some(&sha256_hex("1\n")))},
            {"op_id":"a2","kind":"add","path_after":"a2.txt","content":"2\n","restored_preview_hash":sha256_hex("2\n"),"full_file_scope":true,"per_op_proof":proof("pa2", None, Some(&sha256_hex("2\n")))},
            {"op_id":"a3","kind":"add","path_after":"a3.txt","content":"3\n","restored_preview_hash":sha256_hex("3\n"),"full_file_scope":true,"per_op_proof":proof("pa3", None, Some(&sha256_hex("3\n")))}
        ]
    })).unwrap()).unwrap();
    let refactor = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "refactor-plan",
            "--payload",
            refactor_path.to_str().unwrap(),
            "--chunk-size",
            "2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        refactor.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&refactor.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&refactor.stdout).unwrap();
    assert_eq!(json["chunk_count"], 2);
    assert_eq!(
        json["chunks"][0]["operation_ids"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn workspace_fuzzy_requires_base_and_preview_proofs() {
    let dir = tempfile::tempdir().unwrap();
    let old = "alpha\nneedle\nomega\n";
    let new = "alpha\nreplacement\nomega\n";
    std::fs::write(dir.path().join("a.txt"), old).unwrap();

    let missing_preview = dir.path().join("missing-preview.json");
    std::fs::write(&missing_preview, serde_json::to_string(&serde_json::json!({
        "plan_id":"missing-preview",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "policy":{"allow_fuzzy_apply":true},
        "operations":[{"op_id":"f1","kind":"modify_fuzzy","path_before":"a.txt","base_file_hash":sha256_hex(old),"anchor_before":"needle","replacement":"replacement","expected_occurrences":1,"confidence":0.99,"full_file_scope":true,"per_op_proof":proof("pf", Some(&sha256_hex(old)), None)}]
    })).unwrap()).unwrap();
    let bad_preview = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            missing_preview.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad_preview.status.success());
    assert!(String::from_utf8_lossy(&bad_preview.stderr).contains("requires restored_preview_hash"));

    let missing_base = dir.path().join("missing-base.json");
    std::fs::write(&missing_base, serde_json::to_string(&serde_json::json!({
        "plan_id":"missing-base",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "policy":{"allow_fuzzy_apply":true},
        "operations":[{"op_id":"f2","kind":"modify_fuzzy","path_before":"a.txt","anchor_before":"needle","replacement":"replacement","expected_occurrences":1,"confidence":0.99,"restored_preview_hash":sha256_hex(new),"full_file_scope":true,"per_op_proof":proof("pf2", None, Some(&sha256_hex(new)))}]
    })).unwrap()).unwrap();
    let bad_base = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            missing_base.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad_base.status.success());
    assert!(String::from_utf8_lossy(&bad_base.stderr).contains("requires base_file_hash"));
}

#[test]
fn workspace_rejects_multiple_same_file_mutations_until_range_apply_exists() {
    let dir = tempfile::tempdir().unwrap();
    let old = "abcdef\n";
    std::fs::write(dir.path().join("a.txt"), old).unwrap();
    let plan_path = dir.path().join("same-file.json");
    std::fs::write(&plan_path, serde_json::to_string(&serde_json::json!({
        "plan_id":"same-file",
        "origin":{"kind":"agent_runtime","host":"generic","invocation":"wrapper","intercepted":true,"user_shell_mutated":false},
        "operations":[
            {"op_id":"m1","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex(old),"content":"one\n","selected_range":{"byte_start":0,"byte_end":2},"per_op_proof":proof("p1", Some(&sha256_hex(old)), None)},
            {"op_id":"m2","kind":"modify_exact","path_before":"a.txt","base_file_hash":sha256_hex(old),"content":"two\n","selected_range":{"byte_start":3,"byte_end":5},"per_op_proof":proof("p2", Some(&sha256_hex(old)), None)}
        ]
    })).unwrap()).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args([
            "workspace",
            "validate",
            "--payload",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("both mutate"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        old
    );
}

#[test]
fn restore_patch_restores_compact_patch_text_without_apply_authority() {
    let mut payload = tempfile::NamedTempFile::new().unwrap();
    write!(
        payload,
        "{}",
        serde_json::json!({
            "scope_id":"patch-scope",
            "language":"javascript",
            "compactness":"symbol",
            "patch":"--- old/sum.js\n+++ new/sum.js\n@@\n-function f0(u,v){return u+v;}\n+function f0(u,v){const w=u+v;return w;}\n",
            "symbols":{"calculateTotal":"f0","left":"u","right":"v","total":"w"}
        })
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args([
            "restore-patch",
            "--payload",
            payload.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["canonical_for"], "file_write_and_user_display");
    let restored = json["file_code"].as_str().unwrap();
    assert!(restored.contains("--- old/sum.js"), "{restored}");
    assert!(restored.contains("+++ new/sum.js"), "{restored}");
    assert!(restored.contains("calculateTotal"), "{restored}");
    assert!(restored.contains("left"), "{restored}");
    assert!(restored.contains("right"), "{restored}");
    assert!(restored.contains("total"), "{restored}");
}
