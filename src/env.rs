use std::path::PathBuf;

/// Resolved environment inputs that drive credential lookup.
///
/// Constructed from the process environment via [`Env::from_process`] in
/// production, or built directly in tests so resolution can be exercised
/// without mutating global env vars.
#[derive(Debug, Clone, Default)]
pub struct Env {
    /// User's home directory. Drives default Claude/Codex paths.
    pub home: Option<PathBuf>,
    /// Current login name. Used as the default macOS keychain `account` filter.
    pub user: Option<String>,
    /// `$CLAUDE_CONFIG_DIR` — when set, `.credentials.json` is read from here.
    pub claude_config_dir: Option<PathBuf>,
    /// `$CLAUDE_CREDENTIALS_FILE` — direct override of the credentials file path.
    pub claude_credentials_file: Option<PathBuf>,
    /// `$CODEX_HOME` — when set, `auth.json` is read from here.
    pub codex_home: Option<PathBuf>,
    /// `$CODEX_AUTH_FILE` — direct override of the codex auth file path.
    pub codex_auth_file: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Self {
        Self {
            home: dirs::home_dir(),
            // USER is Unix; USERNAME is the Windows equivalent.
            user: std::env::var("USER")
                .ok()
                .or_else(|| std::env::var("USERNAME").ok()),
            claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            claude_credentials_file: std::env::var_os("CLAUDE_CREDENTIALS_FILE").map(PathBuf::from),
            codex_home: std::env::var_os("CODEX_HOME").map(PathBuf::from),
            codex_auth_file: std::env::var_os("CODEX_AUTH_FILE").map(PathBuf::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none() {
        let e = Env::default();
        assert!(e.home.is_none());
        assert!(e.user.is_none());
        assert!(e.claude_config_dir.is_none());
        assert!(e.claude_credentials_file.is_none());
        assert!(e.codex_home.is_none());
        assert!(e.codex_auth_file.is_none());
    }

    #[test]
    fn from_process_does_not_panic() {
        let _ = Env::from_process();
    }
}
