use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand, ValueEnum};

use agentkey::env::Env;
use agentkey::{claude, codex};

#[derive(Parser, Debug)]
#[command(
    name = "agentkey",
    version,
    arg_required_else_help = true,
    about = "Locate and print Claude/Codex OAuth tokens stashed on this machine.",
    long_about = "Resolution order:\n  Claude → \
        $CLAUDE_CREDENTIALS_FILE → $CLAUDE_CONFIG_DIR/.credentials.json → \
        ~/.claude/.credentials.json → macOS keychain \"Claude Code-credentials\"\n  Codex  → \
        $CODEX_AUTH_FILE → $CODEX_HOME/auth.json → ~/.codex/auth.json"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print refresh tokens. Default: both providers, prefixed.
    Show {
        /// Restrict output to a single provider.
        provider: Option<Provider>,
        /// Include access tokens, expiry, scopes, and account metadata.
        #[arg(short = 'a', long)]
        access: bool,
        /// Print the resolved credential as JSON.
        #[arg(short = 'j', long, conflicts_with = "access")]
        json: bool,
    },
    /// Copy refresh token(s) to the system clipboard.
    Copy {
        /// Restrict to a single provider.
        provider: Option<Provider>,
    },
    /// Show every candidate path agentkey probes and which one resolved.
    Doctor {
        /// Restrict diagnostics to a single provider.
        provider: Option<Provider>,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Provider {
    Claude,
    Codex,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.command) {
        eprintln!("agentkey: {e}");
        std::process::exit(1);
    }
}

fn run(cmd: Cmd) -> Result<(), String> {
    let env = Env::from_process();
    match cmd {
        Cmd::Show {
            provider,
            access,
            json,
        } => show(provider, access, json, &env),
        Cmd::Copy { provider } => copy(provider, &env),
        Cmd::Doctor { provider } => doctor(provider, &env),
    }
}

fn providers_from_filter(filter: Option<Provider>) -> Vec<Provider> {
    match filter {
        Some(p) => vec![p],
        None => vec![Provider::Claude, Provider::Codex],
    }
}

fn show(filter: Option<Provider>, access: bool, json: bool, env: &Env) -> Result<(), String> {
    let providers = providers_from_filter(filter);
    let prefixed = filter.is_none();
    let mut first = true;
    for p in providers {
        if !first {
            println!();
        }
        first = false;
        if json {
            print_json(p, env)?;
        } else if access {
            print_access(p, env)?;
        } else {
            print_refresh(p, env, prefixed)?;
        }
    }
    Ok(())
}

fn copy(filter: Option<Provider>, env: &Env) -> Result<(), String> {
    let providers = providers_from_filter(filter);
    let mut tokens = Vec::with_capacity(providers.len());
    for p in &providers {
        tokens.push(refresh_token(*p, env)?);
    }
    copy_to_clipboard(&tokens.join("\n"))?;
    eprintln!(
        "agentkey: copied {} refresh token(s) to clipboard",
        tokens.len()
    );
    Ok(())
}

fn refresh_token(p: Provider, env: &Env) -> Result<String, String> {
    Ok(match p {
        Provider::Claude => claude::find(env)?.0.refresh_token,
        Provider::Codex => codex::find(env)?.0.tokens.refresh_token,
    })
}

fn print_refresh(p: Provider, env: &Env, prefixed: bool) -> Result<(), String> {
    let tok = refresh_token(p, env)?;
    if prefixed {
        let label = match p {
            Provider::Claude => "claude",
            Provider::Codex => "codex ",
        };
        println!("{label}: {tok}");
    } else {
        println!("{tok}");
    }
    Ok(())
}

fn print_access(p: Provider, env: &Env) -> Result<(), String> {
    match p {
        Provider::Claude => {
            let (c, src) = claude::find(env)?;
            let expiry = Utc
                .timestamp_millis_opt(c.expires_at)
                .single()
                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "<invalid>".into());
            println!("# claude");
            println!("source:  {src}");
            println!("access:  {}", c.access_token);
            println!("refresh: {}", c.refresh_token);
            println!("expires: {} ({expiry})", c.expires_at);
            println!("scopes:  {}", c.scopes.join(","));
            if let Some(s) = c.subscription_type {
                println!("plan:    {s}");
            }
        }
        Provider::Codex => {
            let (c, src) = codex::find(env)?;
            println!("# codex");
            println!("source:  {src}");
            println!("access:  {}", c.tokens.access_token);
            println!("refresh: {}", c.tokens.refresh_token);
            println!("account: {}", c.tokens.account_id);
            if let Some(lr) = c.last_refresh {
                println!("last_refresh: {lr}");
            }
        }
    }
    Ok(())
}

fn print_json(p: Provider, env: &Env) -> Result<(), String> {
    use serde_json::json;
    let v = match p {
        Provider::Claude => {
            let (c, _) = claude::find(env)?;
            json!({
                "accessToken": c.access_token,
                "refreshToken": c.refresh_token,
                "expiresAt": c.expires_at,
                "scopes": c.scopes,
                "subscriptionType": c.subscription_type,
            })
        }
        Provider::Codex => {
            let (c, _) = codex::find(env)?;
            json!({
                "access_token": c.tokens.access_token,
                "refresh_token": c.tokens.refresh_token,
                "account_id": c.tokens.account_id,
                "last_refresh": c.last_refresh,
            })
        }
    };
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
    Ok(())
}

fn doctor(filter: Option<Provider>, env: &Env) -> Result<(), String> {
    use agentkey::source::Source;
    let providers = providers_from_filter(filter);
    for (i, p) in providers.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let label = match p {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        };
        println!("# {label}");
        let paths = match p {
            Provider::Claude => claude::candidate_paths(env),
            Provider::Codex => codex::candidate_paths(env),
        };
        if paths.is_empty() {
            println!("  (no file candidates — $HOME/$CODEX_HOME/$CLAUDE_CONFIG_DIR all unset)");
        } else {
            for path in &paths {
                let mark = if path.exists() { "✓" } else { "·" };
                println!("  {mark} {}", path.display());
            }
        }
        #[cfg(target_os = "macos")]
        if matches!(p, Provider::Claude) {
            println!("  · macOS keychain service \"{}\"", claude::KEYCHAIN_SERVICE);
        }
        let resolved = match p {
            Provider::Claude => claude::find(env).map(|(_, s)| s),
            Provider::Codex => codex::find(env).map(|(_, s)| s),
        };
        match resolved {
            Ok(Source::File(p)) => println!("  → resolved: file {}", p.display()),
            Ok(s) => println!("  → resolved: {s}"),
            Err(e) => println!("  → not found: {}", e.lines().next().unwrap_or("")),
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_to_clipboard(s: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn pbcopy: {e}"))?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(s.as_bytes())
        .map_err(|e| format!("write pbcopy: {e}"))?;
    let st = child.wait().map_err(|e| format!("wait pbcopy: {e}"))?;
    if !st.success() {
        return Err(format!("pbcopy exited {st}"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_to_clipboard(s: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];
    for (cmd, args) in candidates {
        let mut spawn = Command::new(cmd);
        spawn
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let Ok(mut child) = spawn.spawn() else {
            continue;
        };
        if let Some(stdin) = child.stdin.as_mut() {
            if stdin.write_all(s.as_bytes()).is_err() {
                continue;
            }
        }
        if let Ok(st) = child.wait() {
            if st.success() {
                return Ok(());
            }
        }
    }
    Err("no clipboard tool found (install wl-copy, xclip, or xsel)".into())
}

#[cfg(target_os = "windows")]
fn copy_to_clipboard(s: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("clip")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn clip: {e}"))?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(s.as_bytes())
        .map_err(|e| format!("write clip: {e}"))?;
    let st = child.wait().map_err(|e| format!("wait clip: {e}"))?;
    if !st.success() {
        return Err(format!("clip exited {st}"));
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn copy_to_clipboard(_s: &str) -> Result<(), String> {
    Err("clipboard not supported on this platform".into())
}
