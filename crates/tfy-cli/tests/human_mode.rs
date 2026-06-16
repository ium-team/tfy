use std::process::Command;

fn run_tfy(args: &[&str], dir: &tempfile::TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tfy"))
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn human_run_preserves_plain_text_for_small_output() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tfy(
        &[
            "human",
            "run",
            "--raw-dir",
            ".tfy/raw",
            "--ledger",
            ".tfy/human/ledger.jsonl",
            "--",
            "sh",
            "-c",
            "printf ok",
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    assert!(dir.path().join(".tfy/raw").exists());
    assert!(dir.path().join(".tfy/human/ledger.jsonl").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn human_run_summarizes_long_output_and_records_human_route_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tfy(
        &[
            "human",
            "run",
            "--raw-dir",
            ".tfy/raw",
            "--ledger",
            ".tfy/human/ledger.jsonl",
            "--",
            "sh",
            "-c",
            "for i in $(seq 1 80); do echo line $i; done",
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("TFY command summary"), "{text}");
    assert!(text.contains("raw_ref="), "{text}");

    let json_output = run_tfy(
        &[
            "human",
            "run",
            "--json",
            "--raw-dir",
            ".tfy/raw-json",
            "--ledger",
            ".tfy/human/ledger-json.jsonl",
            "--",
            "sh",
            "-c",
            "printf ok",
        ],
        &dir,
    );
    assert!(
        json_output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(json["origin"]["kind"], "human_cli");
    assert_eq!(json["origin"]["invocation"], "wrapper");
    assert_eq!(json["origin"]["intercepted"], true);
    assert_eq!(json["route"]["ingress"], "human_managed_session");
}

#[cfg(not(target_os = "linux"))]
#[test]
fn human_install_fails_closed_on_non_linux() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tfy(&["human", "install", "--dry-run"], &dir);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Linux bash only"));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn human_run_fails_closed_on_non_linux() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tfy(
        &[
            "human",
            "run",
            "--raw-dir",
            ".tfy/raw",
            "--ledger",
            ".tfy/human/ledger.jsonl",
            "--",
            "sh",
            "-c",
            "printf ok",
        ],
        &dir,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Linux bash only"));
    assert!(!dir.path().join(".tfy/human/ledger.jsonl").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn human_install_generates_owned_bash_wrapper_and_refuses_non_tfy_uninstall() {
    use std::fs;
    use std::os::unix::fs as unix_fs;

    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("human.bashrc");
    let output = run_tfy(
        &[
            "human",
            "install",
            "--output",
            script.to_str().unwrap(),
            "--raw-dir",
            ".tfy/raw",
            "--ledger",
            ".tfy/human/ledger.jsonl",
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let install_stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        install_stdout.contains("scope=project_scoped_tfy_managed_session"),
        "{install_stdout}"
    );
    let script_text = fs::read_to_string(&script).unwrap();
    assert!(script_text.contains("TFY:HUMAN-SESSION:START"));
    assert!(script_text.contains("_tfy_human_run"));
    assert!(script_text.contains("TFY_LAST_STATUS"));
    assert!(!script_text.contains("shopt -s extdebug"));
    assert!(!script_text.contains("tfy human capture"));
    assert!(script_text.contains("sh()"));

    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let tfy_link = bin.join("tfy");
    unix_fs::symlink(env!("CARGO_BIN_EXE_tfy"), &tfy_link).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new("bash")
        .current_dir(dir.path())
        .arg("-lc")
        .arg(format!("source {}; sh -c 'printf ok'", script.display()))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&smoke.stdout), "ok");
    assert!(dir.path().join(".tfy/raw").exists());
    assert!(dir.path().join(".tfy/human/ledger.jsonl").exists());

    let subdir = dir.path().join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let scoped = Command::new("bash")
        .current_dir(dir.path())
        .arg("-lc")
        .arg(format!(
            "source {}; cd {}; sh -c 'printf inside'; cd {}; sh -c 'printf outside'; sh -c 'exit 7'; printf ' status=%s' \"$TFY_LAST_STATUS\"",
            script.display(),
            subdir.display(),
            outside.path().display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        scoped.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&scoped.stdout),
        "insideoutside status=7"
    );
    assert!(
        !outside.path().join(".tfy/human/ledger.jsonl").exists(),
        "outside cwd must not receive TFY human ledger"
    );
    let ledger_text = fs::read_to_string(dir.path().join(".tfy/human/ledger.jsonl")).unwrap();
    assert!(
        ledger_text.contains("inside") || ledger_text.contains("human_managed_session"),
        "{ledger_text}"
    );

    let non_tfy = dir.path().join("other.bashrc");
    fs::write(&non_tfy, "echo nope\n").unwrap();
    let uninstall = run_tfy(
        &["human", "uninstall", "--path", non_tfy.to_str().unwrap()],
        &dir,
    );
    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("refusing to remove"));

    let uninstall = run_tfy(
        &["human", "uninstall", "--path", script.to_str().unwrap()],
        &dir,
    );
    assert!(
        uninstall.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(!script.exists());
}
