# agentkey

A small Rust CLI for locating and printing the OAuth credentials that the
[Claude Code](https://docs.claude.com/en/docs/claude-code/overview) and
[OpenAI Codex](https://github.com/openai/codex) CLIs stash on your machine.

If you've signed in to either tool, `agentkey` finds the active session and
prints the access token, refresh token, expiry, scopes, and provider account
metadata — useful for piping into scripts, environment variables, or quick
inspection without grepping the keychain or `cat`-ing config files.

> agentkey only **reads** credentials that already exist locally — it never
> performs OAuth flows, contacts auth servers, or writes anywhere.

## Install

```bash
cargo install --git https://github.com/winterrx/agentkey
# or, from a clone:
cargo install --path .
```

The binary lands at `~/.cargo/bin/agentkey`.

## Usage

```text
agentkey                 # prints help
agentkey show            # both refresh tokens (prefixed)
agentkey show claude     # claude refresh token, raw
agentkey show codex      # codex refresh token, raw
agentkey show --access   # access token, expiry, scopes, account
agentkey show --json     # machine-readable JSON
agentkey copy            # copy refresh token(s) to system clipboard
agentkey doctor          # show every candidate path and which one resolved
agentkey help <cmd>
```

Pipe-friendly:

```bash
export ANTHROPIC_REFRESH_TOKEN=$(agentkey show claude)
agentkey show codex | pbcopy
```

## Resolution order

`agentkey` probes credentials in priority order and stops at the first match.

**Claude**
1. `$CLAUDE_CREDENTIALS_FILE` — explicit file override
2. `$CLAUDE_CONFIG_DIR/.credentials.json`
3. `~/.claude/.credentials.json` *(Linux / Windows default; sometimes macOS)*
4. macOS keychain item `Claude Code-credentials` *(macOS only)*

**Codex**
1. `$CODEX_AUTH_FILE` — explicit file override
2. `$CODEX_HOME/auth.json`
3. `~/.codex/auth.json`

`agentkey doctor` prints every candidate, marks the ones that exist, and
shows which one was selected — useful when a token "isn't being found."

## Platform support

| Platform | Claude source                     | Codex source            | Clipboard |
|----------|-----------------------------------|-------------------------|-----------|
| macOS    | Keychain (preferred) + file       | `~/.codex/auth.json`    | `pbcopy`  |
| Linux    | `~/.claude/.credentials.json`     | `~/.codex/auth.json`    | `wl-copy` / `xclip` / `xsel` |
| Windows  | `%USERPROFILE%/.claude/.credentials.json` | `%USERPROFILE%/.codex/auth.json` | `clip` |

Username is read from `$USER` (Unix) or `$USERNAME` (Windows). Home is
resolved via the [`dirs`](https://crates.io/crates/dirs) crate. Nothing is
hardcoded.

## Build & test

```bash
cargo build --release
cargo test                # 27 unit tests + 13 integration tests
cargo install --path .
```

CLI parsing uses [`clap`](https://crates.io/crates/clap) with derive macros —
the de facto Rust CLI framework.

## Security

These OAuth credentials are long-lived and tied to your provider account.
`agentkey` prints them to stdout by design; treat the output like a password:

- Don't paste tokens into chats, screen-shares, or third-party services.
- Prefer `agentkey copy` over leaving them in your scrollback.
- If a token leaks, sign out and back in to rotate (`claude` / `codex login`).

## License

MIT — see [LICENSE](LICENSE).
