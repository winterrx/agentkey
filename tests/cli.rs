//! End-to-end tests against the compiled `agentkey` binary.
//!
//! These tests drive resolution entirely through `$CLAUDE_CREDENTIALS_FILE`
//! and `$CODEX_AUTH_FILE` overrides, which are direct path inputs and
//! therefore portable across macOS, Linux, and Windows without depending on
//! `$HOME` resolution semantics that vary by platform.

use std::process::Command;

const CLAUDE_WRAPPED: &str = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-XYZ","refreshToken":"sk-ant-ort01-WWW","expiresAt":1777499475796,"scopes":["user:profile","user:inference"],"subscriptionType":"max"}}"#;

const CLAUDE_BARE: &str = r#"{"accessToken":"sk-ant-oat01-AAA","refreshToken":"sk-ant-ort01-BARE","expiresAt":1700000000000,"scopes":["user:profile"]}"#;

const CODEX_FULL: &str = r#"{"auth_mode":"chatgpt","tokens":{"id_token":"id-x","access_token":"acc-y","refresh_token":"rt_zzz","account_id":"acct-1"},"last_refresh":"2026-04-25T21:48:34Z"}"#;

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agentkey"));
    // Strip every credential-related env var so the parent shell's real
    // tokens cannot leak into the test and skew assertions.
    for k in [
        "CLAUDE_CREDENTIALS_FILE",
        "CLAUDE_CONFIG_DIR",
        "CODEX_AUTH_FILE",
        "CODEX_HOME",
    ] {
        cmd.env_remove(k);
    }
    cmd
}

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn bare_invocation_prints_help_and_exits_nonzero() {
    // arg_required_else_help = true makes clap print help to stderr with a
    // non-zero exit code when no subcommand is given (matches agentgrep).
    let out = bin().output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Usage:"));
    assert!(stderr.contains("Commands:"));
    assert!(stderr.contains("show"));
    assert!(stderr.contains("doctor"));
}

#[test]
fn version_flag_works() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("agentkey"));
}

#[test]
fn help_includes_resolution_order() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("CLAUDE_CREDENTIALS_FILE"));
    assert!(stdout.contains("CODEX_AUTH_FILE"));
}

#[test]
fn show_default_prints_both_with_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let claude_file = write(dir.path(), "claude.json", CLAUDE_WRAPPED);
    let codex_file = write(dir.path(), "codex.json", CODEX_FULL);
    let out = bin()
        .arg("show")
        .env("CLAUDE_CREDENTIALS_FILE", &claude_file)
        .env("CODEX_AUTH_FILE", &codex_file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("claude: sk-ant-ort01-WWW"));
    assert!(stdout.contains("codex : rt_zzz"));
}

#[test]
fn show_single_provider_prints_raw_token_without_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let claude_file = write(dir.path(), "claude.json", CLAUDE_BARE);
    let out = bin()
        .args(["show", "claude"])
        .env("CLAUDE_CREDENTIALS_FILE", &claude_file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "sk-ant-ort01-BARE");
}

#[test]
fn show_access_includes_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let claude_file = write(dir.path(), "claude.json", CLAUDE_WRAPPED);
    let out = bin()
        .args(["show", "claude", "--access"])
        .env("CLAUDE_CREDENTIALS_FILE", &claude_file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("# claude"));
    assert!(stdout.contains("access:  sk-ant-oat01-XYZ"));
    assert!(stdout.contains("refresh: sk-ant-ort01-WWW"));
    assert!(stdout.contains("expires: 1777499475796"));
    assert!(stdout.contains("scopes:  user:profile,user:inference"));
    assert!(stdout.contains("plan:    max"));
    assert!(stdout.contains("source:  file:"));
}

#[test]
fn show_json_emits_pretty_json() {
    let dir = tempfile::tempdir().unwrap();
    let codex_file = write(dir.path(), "codex.json", CODEX_FULL);
    let out = bin()
        .args(["show", "codex", "--json"])
        .env("CODEX_AUTH_FILE", &codex_file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(v["refresh_token"], "rt_zzz");
    assert_eq!(v["access_token"], "acc-y");
    assert_eq!(v["account_id"], "acct-1");
}

#[test]
fn show_access_and_json_are_mutually_exclusive() {
    let out = bin()
        .args(["show", "claude", "--access", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("cannot be used") || stderr.contains("conflict"));
}

#[test]
fn doctor_lists_candidates_and_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let claude_file = write(dir.path(), "c.json", CLAUDE_BARE);
    let codex_file = write(dir.path(), "k.json", CODEX_FULL);
    let out = bin()
        .arg("doctor")
        .env("CLAUDE_CREDENTIALS_FILE", &claude_file)
        .env("CODEX_AUTH_FILE", &codex_file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("# claude"));
    assert!(stdout.contains("# codex"));
    assert!(stdout.contains(claude_file.to_str().unwrap()));
    assert!(stdout.contains(codex_file.to_str().unwrap()));
    assert!(stdout.contains("→ resolved: file"));
}

#[test]
fn doctor_for_single_provider_only() {
    let out = bin().args(["doctor", "codex"]).output().unwrap();
    // Even when nothing resolves, doctor itself returns success — it's a
    // diagnostic, not a fetch.
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("# codex"));
    assert!(!stdout.contains("# claude"));
}

#[test]
fn show_missing_credentials_exits_nonzero_with_helpful_error() {
    // Point HOME at an empty temp dir too — otherwise the binary will fall
    // through CODEX_AUTH_FILE and resolve the real ~/.codex/auth.json on
    // the developer machine, masking the negative case.
    let dir = tempfile::tempdir().unwrap();
    let bogus = dir.path().join("does-not-exist.json");
    let out = bin()
        .args(["show", "codex"])
        .env("CODEX_AUTH_FILE", &bogus)
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected failure; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn show_malformed_json_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let f = write(dir.path(), "bad.json", "{not json");
    let out = bin()
        .args(["show", "codex"])
        .env("CODEX_AUTH_FILE", &f)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("parse codex"));
}

#[test]
fn explicit_file_env_var_wins_over_implicit_config_dir() {
    let dir = tempfile::tempdir().unwrap();
    let real = write(dir.path(), "real.json", CLAUDE_WRAPPED);
    let out = bin()
        .args(["show", "claude"])
        .env("CLAUDE_CREDENTIALS_FILE", &real)
        .env("CLAUDE_CONFIG_DIR", "/var/empty/nonexistent")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "sk-ant-ort01-WWW"
    );
}
