use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::env::Env;
use crate::source::Source;

#[cfg(target_os = "macos")]
pub const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ClaudeOAuth {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    pub scopes: Vec<String>,
    #[serde(rename = "subscriptionType", default)]
    pub subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct Wrap {
    #[serde(rename = "claudeAiOauth")]
    oauth: ClaudeOAuth,
}

/// Parse either the wrapped keychain payload `{"claudeAiOauth": {...}, ...}`
/// or a bare `ClaudeOAuth` object (some platforms write the file unwrapped).
pub fn parse(raw: &str) -> Result<ClaudeOAuth, String> {
    if let Ok(w) = serde_json::from_str::<Wrap>(raw) {
        return Ok(w.oauth);
    }
    serde_json::from_str::<ClaudeOAuth>(raw)
        .map_err(|e| format!("parse claude credentials: {e}"))
}

/// Ordered file candidates to probe for Claude credentials. Lower index wins.
pub fn candidate_paths(env: &Env) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = &env.claude_credentials_file {
        v.push(p.clone());
    }
    if let Some(d) = &env.claude_config_dir {
        v.push(d.join(".credentials.json"));
    }
    if let Some(h) = &env.home {
        v.push(h.join(".claude").join(".credentials.json"));
    }
    v
}

pub fn find(env: &Env) -> Result<(ClaudeOAuth, Source), String> {
    for p in candidate_paths(env) {
        if p.exists() {
            let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
            return Ok((parse(&raw)?, Source::File(p)));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some((raw, account)) = read_macos_keychain(env)? {
            return Ok((
                parse(&raw)?,
                Source::Keychain {
                    service: KEYCHAIN_SERVICE.into(),
                    account,
                },
            ));
        }
    }
    Err(not_found_message(env))
}

#[cfg(target_os = "macos")]
fn read_macos_keychain(env: &Env) -> Result<Option<(String, Option<String>)>, String> {
    use std::process::Command;
    // Try the current user's account first; fall back to a service-only lookup
    // so credentials provisioned under a different login name still resolve.
    let attempts: Vec<Option<&str>> = match env.user.as_deref() {
        Some(u) => vec![Some(u), None],
        None => vec![None],
    };
    for account in attempts {
        let mut cmd = Command::new("security");
        cmd.args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"]);
        if let Some(a) = account {
            cmd.args(["-a", a]);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("invoke `security`: {e}"))?;
        if out.status.success() {
            let s = String::from_utf8(out.stdout)
                .map_err(|e| format!("keychain payload utf-8: {e}"))?;
            return Ok(Some((s.trim().to_string(), account.map(String::from))));
        }
    }
    Ok(None)
}

fn not_found_message(env: &Env) -> String {
    let mut msg = String::from("claude credentials not found.\nLooked in:\n");
    for p in candidate_paths(env) {
        msg.push_str(&format!("  - {}\n", p.display()));
    }
    #[cfg(target_os = "macos")]
    msg.push_str(&format!(
        "  - macOS keychain service \"{}\"\n",
        KEYCHAIN_SERVICE
    ));
    msg.push_str("Sign in with `claude` to create credentials.");
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const KEYCHAIN_SAMPLE: &str = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-AAA","refreshToken":"sk-ant-ort01-BBB","expiresAt":1777499475796,"scopes":["user:profile","user:inference"],"subscriptionType":"max"},"mcpOAuth":{}}"#;
    const BARE_SAMPLE: &str = r#"{"accessToken":"sk-ant-oat01-CCC","refreshToken":"sk-ant-ort01-DDD","expiresAt":1700000000000,"scopes":["user:profile"]}"#;

    #[test]
    fn parse_wrapped_keychain_blob() {
        let o = parse(KEYCHAIN_SAMPLE).unwrap();
        assert_eq!(o.access_token, "sk-ant-oat01-AAA");
        assert_eq!(o.refresh_token, "sk-ant-ort01-BBB");
        assert_eq!(o.expires_at, 1777499475796);
        assert_eq!(o.scopes, vec!["user:profile", "user:inference"]);
        assert_eq!(o.subscription_type.as_deref(), Some("max"));
    }

    #[test]
    fn parse_bare_oauth_object() {
        let o = parse(BARE_SAMPLE).unwrap();
        assert_eq!(o.refresh_token, "sk-ant-ort01-DDD");
        assert!(o.subscription_type.is_none());
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse("{}").is_err());
        assert!(parse("not json at all").is_err());
        assert!(parse(r#"{"claudeAiOauth":{}}"#).is_err());
    }

    #[test]
    fn candidate_paths_priority_order() {
        let env = Env {
            home: Some(PathBuf::from("/tmp/h")),
            claude_config_dir: Some(PathBuf::from("/tmp/cfg")),
            claude_credentials_file: Some(PathBuf::from("/tmp/explicit.json")),
            ..Default::default()
        };
        let paths = candidate_paths(&env);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/explicit.json"),
                PathBuf::from("/tmp/cfg/.credentials.json"),
                PathBuf::from("/tmp/h/.claude/.credentials.json"),
            ]
        );
    }

    #[test]
    fn candidate_paths_with_only_home() {
        let env = Env {
            home: Some(PathBuf::from("/u/bob")),
            ..Default::default()
        };
        assert_eq!(
            candidate_paths(&env),
            vec![PathBuf::from("/u/bob/.claude/.credentials.json")]
        );
    }

    #[test]
    fn candidate_paths_empty_when_nothing_configured() {
        assert!(candidate_paths(&Env::default()).is_empty());
    }

    #[test]
    fn find_via_explicit_credentials_file_env() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("c.json");
        std::fs::File::create(&f)
            .unwrap()
            .write_all(KEYCHAIN_SAMPLE.as_bytes())
            .unwrap();
        let env = Env {
            claude_credentials_file: Some(f.clone()),
            ..Default::default()
        };
        let (o, src) = find(&env).unwrap();
        assert_eq!(o.refresh_token, "sk-ant-ort01-BBB");
        assert_eq!(src, Source::File(f));
    }

    #[test]
    fn find_via_config_dir_env() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".credentials.json"), BARE_SAMPLE).unwrap();
        let env = Env {
            claude_config_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        let (o, _) = find(&env).unwrap();
        assert_eq!(o.refresh_token, "sk-ant-ort01-DDD");
    }

    #[test]
    fn find_via_home_dir() {
        let dir = tempfile::tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), BARE_SAMPLE).unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        let (o, src) = find(&env).unwrap();
        assert_eq!(o.refresh_token, "sk-ant-ort01-DDD");
        match src {
            Source::File(p) => assert!(p.ends_with(".claude/.credentials.json")),
            _ => panic!("expected file source"),
        }
    }

    #[test]
    fn explicit_file_wins_over_home() {
        let dir = tempfile::tempdir().unwrap();
        // home has a bare-shape file
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), BARE_SAMPLE).unwrap();
        // explicit file has the wrapped-shape file
        let explicit = dir.path().join("explicit.json");
        std::fs::write(&explicit, KEYCHAIN_SAMPLE).unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            claude_credentials_file: Some(explicit.clone()),
            ..Default::default()
        };
        let (o, src) = find(&env).unwrap();
        // Wrapped sample's refresh token, proving the explicit file was chosen.
        assert_eq!(o.refresh_token, "sk-ant-ort01-BBB");
        assert_eq!(src, Source::File(explicit));
    }

    #[test]
    fn not_found_lists_all_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        // There's no real `find` failure path on macOS that doesn't also touch
        // the user's keychain, so test the message helper directly.
        let msg = not_found_message(&env);
        assert!(msg.contains(".claude/.credentials.json"));
        assert!(msg.contains("claude credentials not found"));
    }
}
