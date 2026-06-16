use std::path::Path;
use std::process::{Command, Output};

fn tfy_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn assert_no_default_artifacts(dir: &Path) {
    assert!(
        !dir.join(".tfy/state/ledger.jsonl").exists(),
        "raw passthrough must not create default ledger"
    );
    assert!(
        !dir.join(".tfy/raw").exists(),
        "raw passthrough must not create raw store"
    );
}

#[test]
fn shell_without_separator_passes_through_tiny_output_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(tmp.path(), &["shell", "printf", "ok"]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ok");
    assert_eq!(output.stderr, b"");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("TFY command summary"));
    assert!(!stdout.contains("raw_ref="));
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_without_separator_does_not_summarize_long_output() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(
        tmp.path(),
        &[
            "shell",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo line $i; done",
        ],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line 1\n"), "{stdout}");
    assert!(stdout.contains("line 80\n"), "{stdout}");
    assert!(!stdout.contains("TFY command summary"), "{stdout}");
    assert!(!stdout.contains("raw_ref="), "{stdout}");
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_without_separator_passes_dash_arguments_to_command() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("visible.txt"), "ok").unwrap();
    let output = tfy_in(tmp.path(), &["shell", "ls", "-la"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("visible.txt"), "{stdout}");
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_without_separator_keeps_command_arguments_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let json_arg = tfy_in(
        tmp.path(),
        &["shell", "sh", "-c", "printf %s \"$1\"", "_", "--json"],
    );
    assert!(json_arg.status.success());
    assert_eq!(json_arg.stdout, b"--json");

    let separator_arg = tfy_in(tmp.path(), &["shell", "echo", "--", "hi"]);
    assert!(separator_arg.status.success());
    assert_eq!(separator_arg.stdout, b"-- hi\n");
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_without_separator_preserves_launch_failure_exit_and_no_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(tmp.path(), &["shell", "definitely-not-a-command-tfy-test"]);
    assert_eq!(output.status.code(), Some(127));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("command launch failed"), "{stderr}");
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_separator_keeps_gateway_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(tmp.path(), &["shell", "--", "sh", "-c", "printf ok"]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ok");
    assert!(tmp.path().join(".tfy/state/ledger.jsonl").exists());
    assert!(tmp.path().join(".tfy/raw").exists());
}

#[test]
fn shell_separator_long_output_still_summarizes_when_smaller() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(
        tmp.path(),
        &[
            "shell",
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 200); do echo line $i repeated build noise; done",
        ],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TFY command summary"), "{stdout}");
    assert!(stdout.contains("raw_ref="), "{stdout}");
}

#[test]
fn shell_json_separator_keeps_generic_shell_route_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(
        tmp.path(),
        &["shell", "--json", "--", "sh", "-c", "printf ok"],
    );
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["payload"]["kind"], "tool_command");
    assert_eq!(json["payload"]["model_text"], "ok");
    assert_eq!(json["adapter_kind"], "shell");
    assert_eq!(json["route"]["ingress"], "generic_shell_adapter");
    assert_eq!(json["origin"]["invocation"], "wrapper");
}

#[test]
fn shell_tfy_options_without_separator_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tfy_in(tmp.path(), &["shell", "--json", "sh", "-c", "printf ok"]);
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires the '--' command separator"),
        "{stderr}"
    );
    assert_no_default_artifacts(tmp.path());

    let output = tfy_in(
        tmp.path(),
        &["shell", "--raw-dir", "tmp", "sh", "-c", "printf ok"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires the '--' command separator"),
        "{stderr}"
    );
    assert!(
        stderr.contains("tfy shell --raw-dir <dir> -- <command>"),
        "{stderr}"
    );
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn shell_classifier_covers_all_shell_options_declared_in_help() {
    let tmp = tempfile::tempdir().unwrap();
    let help = tfy_in(tmp.path(), &["shell", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    let options = [
        "--json",
        "--jsonl",
        "--raw-dir",
        "--ledger",
        "--session-id",
        "--request-id",
        "--trace-id",
        "--parent-event-id",
        "--max-summary-bytes",
        "--max-output-bytes",
    ];
    for option in options {
        if option != "--max-output-bytes" {
            assert!(
                help.contains(option),
                "shell help missing expected option {option}"
            );
        }
        let output = if matches!(option, "--max-summary-bytes" | "--max-output-bytes") {
            tfy_in(tmp.path(), &["shell", option, "1", "sh", "-c", "printf ok"])
        } else if matches!(
            option,
            "--raw-dir"
                | "--ledger"
                | "--session-id"
                | "--request-id"
                | "--trace-id"
                | "--parent-event-id"
        ) {
            tfy_in(
                tmp.path(),
                &["shell", option, "value", "sh", "-c", "printf ok"],
            )
        } else {
            tfy_in(tmp.path(), &["shell", option, "sh", "-c", "printf ok"])
        };
        assert!(
            !output.status.success(),
            "{option} without separator must fail closed"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("requires the '--' command separator"),
            "{option}: {stderr}"
        );
    }
    assert_no_default_artifacts(tmp.path());
}

#[test]
fn adapter_capabilities_names_separator_for_shell_gateway() {
    let output = Command::new(env!("CARGO_BIN_EXE_tfy"))
        .args(["adapter", "capabilities"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let canonical = json["canonical_execution"].as_array().unwrap();
    assert!(canonical.iter().any(|value| value == "tfy shell --"));
    assert!(!canonical.iter().any(|value| value == "tfy shell"));
}
