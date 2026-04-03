# qnap

[![codecov](https://codecov.io/gh/rvben/qnap-cli/graph/badge.svg)](https://codecov.io/gh/rvben/qnap-cli)

A CLI for QNAP NAS management. It is designed for both interactive use and automation: human-friendly tables by default, stable JSON contracts on inspection commands, and a `schema` command for agent introspection.

Supported platforms: Linux and macOS.

## Installation

```sh
cargo install qnap
```

Or build from source:

```sh
git clone https://github.com/rvben/qnap-cli
cd qnap-cli
make install
```

## Quick Start

```sh
# Save connection settings and password
qnap login --host nas.local --username admin

# Inspect the NAS
qnap info
qnap status
qnap volumes
qnap shares
qnap files ls /Public

# Structured output for automation
qnap status --json
qnap files stat /Public/example.txt --json
```

## Commands

| Command | Description |
|---|---|
| `login` | Save credentials and verify connectivity |
| `info` | Model, firmware, hostname, serial, uptime |
| `status` | CPU, RAM, temperature, uptime |
| `volumes` | Storage volumes and installed disks |
| `shares` | Shared folders |
| `network` | Network adapters, IPs, MACs, DNS (`--json`) |
| `config` | Show saved host, username, TLS settings, and file paths (`--json`) |
| `files ls <PATH>` | List files and directories (`--all`, `-r` recursive, `--json`) |
| `files stat <PATH>` | Normalized metadata plus raw QNAP fields (`--json`) |
| `files find <PATH> <PATTERN>` | Recursive glob search (`*.txt`, `backup*`) (`--json`) |
| `files mkdir <PATH>` | Create a directory |
| `files rm <PATH>...` | Delete one or more files or directories |
| `files mv <SRC> <DST>` | Move or rename a file or directory |
| `files cp <SRC> <DST>` | Copy a file or directory (`--overwrite`) |
| `files upload <LOCAL> <REMOTE_DIR>` | Upload a file or directory (`--overwrite`, `-r` recursive) |
| `files download <REMOTE> [LOCAL]` | Download a file or directory (`-r` recursive) |
| `dump [DIR]` | Save raw API responses for debugging |
| `schema` | Print full command schema |
| `completions <SHELL>` | Print shell completion script (`bash`, `zsh`, `fish`) |

## Global Flags

These flags work on every authenticated command, including `login`.

| Flag | Description |
|---|---|
| `--host <HOST>` | Override the NAS host for one command. HTTPS only. |
| `--username <USERNAME>` | Override the username for one command. |
| `--insecure` | Skip TLS certificate verification for one command. |
| `--secure` | Force TLS certificate verification for one command. |
| `--password-stdin` | Read the password from stdin. |

Examples:

```sh
printf '%s\n' 'secret-password' | qnap --password-stdin info
qnap --host https://nas.local --username admin status
qnap --secure volumes --json
```

## Authentication

```sh
qnap login [--host <HOST>] [--username <USERNAME>] [--insecure | --secure]
```

`qnap login` verifies the credentials before saving anything. Host, username, and the saved TLS verification preference are stored in `config.toml`. The password is stored separately in `credentials.toml`.

The CLI requires HTTPS. Plain `http://` targets are rejected. If your NAS uses a self-signed certificate, use `--insecure` or `QNAP_INSECURE=1` explicitly.

If you do not want local password persistence, skip `qnap login` and provide `QNAP_HOST`, `QNAP_USERNAME`, and `QNAP_PASSWORD` directly when running commands.

## JSON Output

The following commands support `--json` with stable, typed output:

- `info`
- `status`
- `volumes`
- `shares`
- `files ls`
- `files stat`

Highlights:

- `status --json` uses numeric fields such as `cpu_usage_pct`, `mem_total_mb`, and `temp_c`.
- `volumes --json` includes `status_code`, `pool_id`, and per-disk `temp_c`.
- `shares --json` uses `items_count` as a number.
- `files ls --json` uses `size_bytes` and `modified_epoch`.
- `files stat --json` returns normalized fields plus a `raw` object for unnormalized QNAP metadata.

Use `qnap schema` for the full machine-readable command and output contract.

## Environment Variables

Environment variables override local files.

| Variable | Description |
|---|---|
| `QNAP_HOST` | NAS host, for example `https://nas.local` or `nas.local` |
| `QNAP_USERNAME` | Username |
| `QNAP_PASSWORD` | Password |
| `QNAP_INSECURE` | `1`/`true`/`yes` to skip TLS verification, `0`/`false`/`no` to force verification |
| `NO_COLOR` | Disable colored output |

## Stored Files

`config.toml` stores host, username, and TLS verification preference. `credentials.toml` stores the password separately in plaintext on the local machine.

| Platform | Config file | Credentials file |
|---|---|---|
| macOS | `~/Library/Application Support/qnap/config.toml` | `~/Library/Application Support/qnap/credentials.toml` |
| Linux | `~/.config/qnap/config.toml` | `~/.config/qnap/credentials.toml` |

## Security

- Passwords are never accepted as CLI flags.
- HTTPS is required for NAS connections.
- TLS verification is enabled by default.
- `credentials.toml` is written atomically.
- On Unix, `credentials.toml` is restricted to owner-only (`0600`) permissions.
- On Unix, the config directory is tightened to owner-only (`0700`) permissions.
- Symlink and non-regular-file credential targets are rejected.
- If you do not want local password persistence, use environment variables or `--password-stdin`.

## Shell Completions

Generate and install completions for your shell:

```sh
# Zsh — add to ~/.zshrc or drop in a $fpath directory
qnap completions zsh > ~/.zfunc/_qnap
echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc

# Bash
qnap completions bash > /etc/bash_completion.d/qnap
# or for a user install:
qnap completions bash > ~/.local/share/bash-completion/completions/qnap

# Fish
qnap completions fish > ~/.config/fish/completions/qnap.fish
```

## Compatibility

Tested against QTS 5.1.x and 5.2.x. The core authentication and file manager APIs have been stable since QTS 4.3, but `qnap dump` is the recommended way to capture compatibility issues from older or unusual firmware builds.

## License

MIT
