use std::fmt;
use std::path::PathBuf;

/// Where a credential was resolved from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    File(PathBuf),
    Keychain {
        service: String,
        account: Option<String>,
    },
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::File(p) => write!(f, "file: {}", p.display()),
            Source::Keychain {
                service,
                account: Some(a),
            } => write!(f, "keychain: {service} (account: {a})"),
            Source::Keychain {
                service,
                account: None,
            } => write!(f, "keychain: {service}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_file() {
        let s = Source::File(PathBuf::from("/tmp/x.json"));
        assert_eq!(s.to_string(), "file: /tmp/x.json");
    }

    #[test]
    fn display_keychain_with_account() {
        let s = Source::Keychain {
            service: "Claude Code-credentials".into(),
            account: Some("alice".into()),
        };
        assert_eq!(
            s.to_string(),
            "keychain: Claude Code-credentials (account: alice)"
        );
    }

    #[test]
    fn display_keychain_without_account() {
        let s = Source::Keychain {
            service: "Claude Code-credentials".into(),
            account: None,
        };
        assert_eq!(s.to_string(), "keychain: Claude Code-credentials");
    }
}
