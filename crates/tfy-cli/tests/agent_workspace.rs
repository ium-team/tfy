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
    assert!(String::from_utf8_lossy(&bad_origin.stderr)
        .contains("requires explicit agent/mcp/test origin"));

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
