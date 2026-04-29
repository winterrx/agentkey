use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::env::Env;
use crate::source::Source;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CodexAuth {
    pub tokens: CodexTokens,
    #[serde(default)]
    pub last_refresh: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CodexTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: String,
}

pub fn parse(raw: &str) -> Result<CodexAuth, String> {
    serde_json::from_str(raw).map_err(|e| format!("parse codex auth.json: {e}"))
}

/// Ordered file candidates to probe for Codex credentials. Lower index wins.
pub fn candidate_paths(env: &Env) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = &env.codex_auth_file {
        v.push(p.clone());
    }
    if let Some(d) = &env.codex_home {
        v.push(d.join("auth.json"));
    }
    if let Some(h) = &env.home {
        v.push(h.join(".codex").join("auth.json"));
    }
    v
}

pub fn find(env: &Env) -> Result<(CodexAuth, Source), String> {
    for p in candidate_paths(env) {
        if p.exists() {
            let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
            return Ok((parse(&raw)?, Source::File(p)));
        }
    }
    Err(not_found_message(env))
}

fn not_found_message(env: &Env) -> String {
    let mut msg = String::from("codex credentials not found.\nLooked in:\n");
    let paths = candidate_paths(env);
    if paths.is_empty() {
        msg.push_str("  (no $HOME, $CODEX_HOME, or $CODEX_AUTH_FILE configured)\n");
    } else {
        for p in paths {
            msg.push_str(&format!("  - {}\n", p.display()));
        }
    }
    msg.push_str("Sign in with `codex login` to create credentials.");
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": "id-AAA",
            "access_token": "acc-BBB",
            "refresh_token": "rt_CCC",
            "account_id": "acct-DDD"
        },
        "last_refresh": "2026-04-25T21:48:34.259669Z"
    }"#;

    #[test]
    fn parse_full_auth_json() {
        let a = parse(SAMPLE).unwrap();
        assert_eq!(a.tokens.access_token, "acc-BBB");
        assert_eq!(a.tokens.refresh_token, "rt_CCC");
        assert_eq!(a.tokens.account_id, "acct-DDD");
        assert_eq!(a.last_refresh.as_deref(), Some("2026-04-25T21:48:34.259669Z"));
    }

    #[test]
    fn parse_without_last_refresh() {
        let raw = r#"{"tokens":{"access_token":"a","refresh_token":"b","account_id":"c"}}"#;
        let a = parse(raw).unwrap();
        assert!(a.last_refresh.is_none());
    }

    #[test]
    fn parse_rejects_missing_required_fields() {
        assert!(parse("{}").is_err());
        assert!(parse(r#"{"tokens":{}}"#).is_err());
        assert!(parse(r#"{"tokens":{"access_token":"a"}}"#).is_err());
    }

    #[test]
    fn candidate_paths_priority_order() {
        let env = Env {
            home: Some(PathBuf::from("/h")),
            codex_home: Some(PathBuf::from("/x")),
            codex_auth_file: Some(PathBuf::from("/f.json")),
            ..Default::default()
        };
        assert_eq!(
            candidate_paths(&env),
            vec![
                PathBuf::from("/f.json"),
                PathBuf::from("/x/auth.json"),
                PathBuf::from("/h/.codex/auth.json"),
            ]
        );
    }

    #[test]
    fn candidate_paths_empty_when_nothing_set() {
        assert!(candidate_paths(&Env::default()).is_empty());
    }

    #[test]
    fn find_via_home() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join(".codex");
        std::fs::create_dir(&codex).unwrap();
        std::fs::write(codex.join("auth.json"), SAMPLE).unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        let (a, src) = find(&env).unwrap();
        assert_eq!(a.tokens.refresh_token, "rt_CCC");
        match src {
            Source::File(p) => assert!(p.ends_with(".codex/auth.json")),
            _ => panic!("expected file source"),
        }
    }

    #[test]
    fn find_via_codex_home_env() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("auth.json"), SAMPLE).unwrap();
        let env = Env {
            codex_home: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        let (a, _) = find(&env).unwrap();
        assert_eq!(a.tokens.refresh_token, "rt_CCC");
    }

    #[test]
    fn find_via_codex_auth_file_env() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("override.json");
        std::fs::write(&f, SAMPLE).unwrap();
        let env = Env {
            codex_auth_file: Some(f.clone()),
            ..Default::default()
        };
        let (_, src) = find(&env).unwrap();
        assert_eq!(src, Source::File(f));
    }

    #[test]
    fn explicit_file_wins_over_codex_home_and_home() {
        let dir = tempfile::tempdir().unwrap();
        // Plant decoys in home/.codex and in codex_home that should be ignored.
        let home_codex = dir.path().join("home/.codex");
        std::fs::create_dir_all(&home_codex).unwrap();
        std::fs::write(
            home_codex.join("auth.json"),
            r#"{"tokens":{"access_token":"a","refresh_token":"DECOY1","account_id":"c"}}"#,
        )
        .unwrap();
        let chome = dir.path().join("chome");
        std::fs::create_dir(&chome).unwrap();
        std::fs::write(
            chome.join("auth.json"),
            r#"{"tokens":{"access_token":"a","refresh_token":"DECOY2","account_id":"c"}}"#,
        )
        .unwrap();
        // The explicit file is what we expect.
        let explicit = dir.path().join("real.json");
        std::fs::write(&explicit, SAMPLE).unwrap();
        let env = Env {
            home: Some(dir.path().join("home")),
            codex_home: Some(chome),
            codex_auth_file: Some(explicit.clone()),
            ..Default::default()
        };
        let (a, src) = find(&env).unwrap();
        assert_eq!(a.tokens.refresh_token, "rt_CCC");
        assert_eq!(src, Source::File(explicit));
    }

    #[test]
    fn not_found_lists_all_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            codex_home: Some(PathBuf::from("/nonexistent")),
            ..Default::default()
        };
        let err = find(&env).unwrap_err();
        assert!(err.contains(".codex/auth.json"));
        assert!(err.contains("/nonexistent/auth.json"));
        assert!(err.contains("codex login"));
    }

    #[test]
    fn not_found_with_no_inputs_explains_missing_env() {
        let err = find(&Env::default()).unwrap_err();
        assert!(err.contains("CODEX_HOME") || err.contains("$HOME"));
    }
}
