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
    assert!(
        !text.contains("tfy custom"),
        "stdout must not include guidance: {text}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tfy custom"), "{stderr}");

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
    assert!(script_text.contains("TFY_HUMAN_SHIM_DIR"));
    assert!(script_text.contains("_tfy_human_refresh_shims"));
    assert!(script_text.contains("TFY_HUMAN_ORIGINAL_PATH"));
    assert!(script_text.contains("_tfy_human_strip_shim_from_path"));
    assert!(script_text.contains("TFY_LAST_STATUS"));
    assert!(script_text.contains("#!/bin/sh"));
    assert!(script_text.contains("command -p chmod 700"));
    assert!(script_text.contains("_tfy_human_validate_shim_dir"));
    assert!(script_text.contains("_tfy_human_disable_shims"));
    assert!(!script_text.contains("\nchmod 700"));
    assert!(!script_text.contains("#!/usr/bin/env sh"));
    assert!(!script_text.contains("shopt -s extdebug"));
    assert!(!script_text.contains("tfy human capture"));
    assert!(!script_text.contains("sh()"));

    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let tfy_link = bin.join("tfy");
    unix_fs::symlink(env!("CARGO_BIN_EXE_tfy"), &tfy_link).unwrap();
    let hello = bin.join("hello-custom");
    fs::write(&hello, "#!/usr/bin/env sh\nprintf ok").unwrap();
    let nested = bin.join("nested-custom");
    fs::write(&nested, "#!/usr/bin/env sh\nhello-custom").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hello, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            "source {}; \"$TFY_HUMAN_SHIM_DIR/hello-custom\"",
            script.display()
        ))
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
        .arg("-c")
        .arg(format!(
            "source {}; cd {}; sh -c 'printf inside'; cd {}; sh -c 'printf outside'",
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
    assert_eq!(String::from_utf8_lossy(&scoped.stdout), "insideoutside");
    assert!(
        !outside.path().join(".tfy/human/ledger.jsonl").exists(),
        "outside cwd must not receive TFY human ledger"
    );
    assert!(dir.path().join(".tfy/raw").exists());
    assert!(dir.path().join(".tfy/human/ledger.jsonl").exists());

    fs::remove_file(dir.path().join(".tfy/human/ledger.jsonl")).unwrap();
    let double_source = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            r#"source {}; source {}; case ":$TFY_HUMAN_ORIGINAL_PATH:" in *":$TFY_HUMAN_SHIM_DIR:"*) exit 9;; esac; hello-custom; test -f .tfy/human/ledger.jsonl; rm .tfy/human/ledger.jsonl; tfy-human-bypass hello-custom; test ! -f .tfy/human/ledger.jsonl; cd {}; "$TFY_HUMAN_SHIM_DIR/hello-custom"; cd {}; hello-custom; test -f .tfy/human/ledger.jsonl"#,
            script.display(),
            script.display(),
            outside.path().display(),
            dir.path().display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        double_source.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&double_source.stdout),
        String::from_utf8_lossy(&double_source.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&double_source.stdout), "okokokok");

    let ledger_text = fs::read_to_string(dir.path().join(".tfy/human/ledger.jsonl")).unwrap();
    assert!(
        ledger_text.contains("human_managed_session"),
        "{ledger_text}"
    );
    assert!(ledger_text.contains("hello-custom"), "{ledger_text}");

    fs::remove_file(dir.path().join(".tfy/human/ledger.jsonl")).unwrap();
    let direct = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            "source {}; ./bin/hello-custom; test ! -f .tfy/human/ledger.jsonl",
            script.display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        direct.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&direct.stderr)
    );

    let refresh_bin = dir.path().join("refresh-bin");
    fs::create_dir_all(&refresh_bin).unwrap();
    let refresh_path = format!("{}:{}", refresh_bin.display(), path);
    let refresh = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            "source {}; export TFY_HUMAN_LEDGER=\"$TFY_HUMAN_ROOT/.tfy/human/refresh-ledger.jsonl\"; printf '#!/usr/bin/env sh\nprintf later' > {}/late-custom; chmod 755 {}/late-custom; late-custom; test ! -f .tfy/human/refresh-ledger.jsonl; _tfy_human_refresh_shims; \"$TFY_HUMAN_SHIM_DIR/late-custom\"; test -f .tfy/human/refresh-ledger.jsonl",
            script.display(),
            refresh_bin.display(),
            refresh_bin.display()
        ))
        .env("PATH", &refresh_path)
        .output()
        .unwrap();
    assert!(
        refresh.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&refresh.stdout),
        String::from_utf8_lossy(&refresh.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&refresh.stdout), "laterlater");

    let nested_smoke = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            "source {}; export TFY_HUMAN_LEDGER=\"$TFY_HUMAN_ROOT/.tfy/human/nested-ledger.jsonl\"; \"$TFY_HUMAN_SHIM_DIR/nested-custom\"",
            script.display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(nested_smoke.status.success());
    let nested_ledger =
        fs::read_to_string(dir.path().join(".tfy/human/nested-ledger.jsonl")).unwrap();
    assert!(nested_ledger.contains("nested-custom"), "{nested_ledger}");
    assert!(!nested_ledger.contains("hello-custom"), "{nested_ledger}");

    let poisoned = tempfile::tempdir().unwrap();
    let poisoned_script = poisoned.path().join("human.bashrc");
    fs::copy(&script, &poisoned_script).unwrap();
    let poisoned_bin = poisoned.path().join(".tfy/human/bin");
    fs::create_dir_all(&poisoned_bin).unwrap();
    let poisoned_shim = poisoned_bin.join("hello-custom");
    fs::write(
        &poisoned_shim,
        "#!/usr/bin/env sh
printf poisoned",
    )
    .unwrap();
    fs::set_permissions(&poisoned_shim, fs::Permissions::from_mode(0o755)).unwrap();
    let poisoned_output = Command::new("bash")
        .current_dir(poisoned.path())
        .arg("-c")
        .arg(format!("source {}", poisoned_script.display()))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        !poisoned_output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&poisoned_output.stdout),
        String::from_utf8_lossy(&poisoned_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&poisoned_output.stderr).contains("refused non-TFY-owned path"),
        "stderr={}",
        String::from_utf8_lossy(&poisoned_output.stderr)
    );

    let forged = tempfile::tempdir().unwrap();
    let forged_script = forged.path().join("human.bashrc");
    fs::copy(&script, &forged_script).unwrap();
    let forged_bin = forged.path().join(".tfy/human/bin");
    fs::create_dir_all(&forged_bin).unwrap();
    let forged_shim = forged_bin.join("hello-custom");
    fs::write(
        &forged_shim,
        "#!/bin/sh
# TFY:HUMAN-SHIM:v1
printf forged",
    )
    .unwrap();
    fs::set_permissions(&forged_shim, fs::Permissions::from_mode(0o755)).unwrap();
    let forged_output = Command::new("bash")
        .current_dir(forged.path())
        .arg("-c")
        .arg(format!(
            "source {}; \"$TFY_HUMAN_SHIM_DIR/hello-custom\"",
            forged_script.display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        forged_output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&forged_output.stdout),
        String::from_utf8_lossy(&forged_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&forged_output.stdout), "ok");
    let rewritten = fs::read_to_string(&forged_shim).unwrap();
    assert!(
        rewritten.contains("exec \"${TFY_HUMAN_TFY_BIN:?}\" human run"),
        "{rewritten}"
    );
    assert!(!rewritten.contains("printf forged"), "{rewritten}");

    let post_start_poison = Command::new("bash")
        .current_dir(dir.path())
        .arg("-c")
        .arg(format!(
            "source {}; command -p cat > .tfy/human/bin/post-poison <<'EOF'\n#!/bin/sh\nprintf latepoison\nEOF\ncommand -p chmod 755 .tfy/human/bin/post-poison; _tfy_human_prompt_command; ! command -v post-poison; ! post-poison",
            script.display()
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        post_start_poison.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&post_start_poison.stdout),
        String::from_utf8_lossy(&post_start_poison.stderr)
    );
    assert!(
        String::from_utf8_lossy(&post_start_poison.stderr).contains("refused non-TFY-owned path"),
        "stderr={}",
        String::from_utf8_lossy(&post_start_poison.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&post_start_poison.stdout).contains("latepoison"),
        "stdout={}",
        String::from_utf8_lossy(&post_start_poison.stdout)
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

#[test]
fn human_auto_activate_rejects_both_target_before_agent_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tfy(&["start", "both", "--auto-activate"], &dir);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("human-only"), "{stderr}");
    assert!(!dir.path().join(".codex/config.toml").exists());
    assert!(!dir.path().join(".tfy/host-config/codex.json").exists());
    assert!(!dir.path().join(".tfy/human/auto-activate.json").exists());
    assert!(!dir.path().join(".tfy/lifecycle.json").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn human_auto_activate_enables_fresh_bash_from_trusted_repo_marker() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let hello = bin.join("hello-auto");
    fs::write(&hello, "#!/usr/bin/env sh\nprintf auto").unwrap();
    fs::set_permissions(&hello, fs::Permissions::from_mode(0o755)).unwrap();

    let enable = run_tfy(&["start", "--human", "--auto-activate", "--no-apply"], &dir);
    assert!(
        enable.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&enable.stdout),
        String::from_utf8_lossy(&enable.stderr)
    );
    assert!(dir.path().join(".tfy/human/auto-activate.json").exists());
    assert!(dir.path().join(".tfy/human/auto-activate.bash").exists());

    let rcfile = dir.path().join("test.bashrc");
    let install = run_tfy(
        &[
            "human",
            "auto-activate",
            "install",
            "--shell",
            "bash",
            "--rcfile",
            rcfile.to_str().unwrap(),
            "--apply",
        ],
        &dir,
    );
    assert!(
        install.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );
    let rc_text = fs::read_to_string(&rcfile).unwrap();
    assert!(rc_text.contains("TFY:HUMAN-AUTO-ACTIVATE:START"));
    assert!(rc_text.contains("TFY_HUMAN_AUTO_ACTIVATE_TFY="));

    let fake = dir.path().join("fake");
    fs::create_dir_all(&fake).unwrap();
    fs::write(
        fake.join("tfy"),
        "#!/usr/bin/env sh\necho fake-tfy-executed >> ../fake-tfy.log\nexit 99\n",
    )
    .unwrap();
    fs::set_permissions(fake.join("tfy"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}:{}",
        fake.display(),
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let smoke = Command::new("bash")
        .current_dir(dir.path())
        .arg("--rcfile")
        .arg(&rcfile)
        .arg("-i")
        .arg("-c")
        .arg("hello-auto; test -f .tfy/human/ledger.jsonl; test \"$TFY_HUMAN_ROOT\" = \"$(pwd -P)\"; test -n \"$TFY_HUMAN_TFY_BIN\"")
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&smoke.stdout), "auto");
    assert!(!dir.path().join("fake-tfy.log").exists());

    let subdir = dir.path().join("sub/dir");
    fs::create_dir_all(&subdir).unwrap();
    let nested = Command::new("bash")
        .current_dir(&subdir)
        .arg("--rcfile")
        .arg(&rcfile)
        .arg("-i")
        .arg("-c")
        .arg(format!(
            "test \"$TFY_HUMAN_ROOT\" = {}; hello-auto",
            shell_escape_for_test(&dir.path().canonicalize().unwrap().display().to_string())
        ))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        nested.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&nested.stdout),
        String::from_utf8_lossy(&nested.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&nested.stdout), "auto");

    let outside = tempfile::tempdir().unwrap();
    let outside_output = Command::new("bash")
        .current_dir(outside.path())
        .arg("--rcfile")
        .arg(&rcfile)
        .arg("-i")
        .arg("-c")
        .arg("test -z \"${TFY_HUMAN_ACTIVE:-}\"")
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        outside_output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&outside_output.stdout),
        String::from_utf8_lossy(&outside_output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn human_auto_activate_rejects_self_certifying_or_symlinked_state() {
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};

    let dir = tempfile::tempdir().unwrap();
    let enable = run_tfy(&["start", "--human", "--auto-activate", "--no-apply"], &dir);
    assert!(enable.status.success());

    fs::write(
        dir.path().join(".tfy/human/auto-activate.bash"),
        "# TFY:HUMAN-AUTO-SCRIPT:START\necho forged\n# TFY:HUMAN-AUTO-SCRIPT:END\n",
    )
    .unwrap();
    let validate = run_tfy(
        &[
            "human",
            "auto-activate",
            "validate",
            "--root",
            dir.path().to_str().unwrap(),
            "--shell",
            "bash",
        ],
        &dir,
    );
    assert!(validate.status.success());
    let repaired = fs::read_to_string(dir.path().join(".tfy/human/auto-activate.bash")).unwrap();
    assert!(!repaired.contains("echo forged"), "{repaired}");
    assert!(repaired.contains("TFY_HUMAN_TFY_BIN"), "{repaired}");

    let symlinked = tempfile::tempdir().unwrap();
    fs::create_dir_all(symlinked.path().join("real/human")).unwrap();
    symlink(symlinked.path().join("real"), symlinked.path().join(".tfy")).unwrap();
    let bad = run_tfy(
        &[
            "human",
            "auto-activate",
            "validate",
            "--root",
            symlinked.path().to_str().unwrap(),
            "--shell",
            "bash",
        ],
        &symlinked,
    );
    assert!(!bad.status.success());

    let rcfile = dir.path().join("hook.bashrc");
    let install = run_tfy(
        &[
            "human",
            "auto-activate",
            "install",
            "--rcfile",
            rcfile.to_str().unwrap(),
            "--apply",
        ],
        &dir,
    );
    assert!(install.status.success());
    fs::set_permissions(&rcfile, fs::Permissions::from_mode(0o644)).unwrap();
    let uninstall = run_tfy(
        &[
            "human",
            "auto-activate",
            "uninstall",
            "--rcfile",
            rcfile.to_str().unwrap(),
            "--apply",
        ],
        &dir,
    );
    assert!(uninstall.status.success());
    assert!(!fs::read_to_string(&rcfile)
        .unwrap()
        .contains("TFY:HUMAN-AUTO-ACTIVATE"));
}

#[cfg(target_os = "linux")]
fn shell_escape_for_test(value: &str) -> String {
    assert!(!value.contains("'"));
    format!("'{value}'")
}
