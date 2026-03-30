# qnap

A CLI for QNAP NAS management. Works well for both humans (colored tables, readable output) and AI agents (`--json` mode, `schema` command for introspection).

## Installation

```sh
cargo install qnap
```

Or build from source:

```sh
git clone https://github.com/rvben/qnap-cli
cd qnap-cli
make install   # builds release binary and copies to ~/.local/bin/qnap
```

## Quick Start

```sh
# Save credentials (password stored in OS keychain, never in config file)
qnap login --host 192.168.1.50 --username admin

# Show system info
qnap info

# Show live resource usage
qnap status

# List volumes and disks
qnap volumes

# List shared folders
qnap shares

# Browse files
qnap files ls /Public

# All commands support --json for structured output
qnap status --json
```

## Commands

| Command | Description |
|---|---|
| `login` | Save credentials and verify connectivity |
| `info` | Model, firmware, hostname, serial, uptime |
| `status` | CPU, RAM, temperature, uptime |
| `volumes` | Storage volumes and installed disks |
| `shares` | Shared folders |
| `files ls <PATH>` | List files and directories |
| `files stat <PATH>` | Metadata for a file or directory |
| `schema` | Print full command schema (for agent use) |

### login

```sh
qnap login [--host <HOST>] [--username <USERNAME>] [--insecure]
```

Prompts for any missing fields. The password is stored in the OS keychain — it is never written to the config file. `--insecure` skips TLS certificate verification, which is useful for QNAP's default self-signed certificates.

### files ls

```sh
qnap files ls /Public
qnap files ls /Backups/photos --json
```

Lists up to 200 items. Paths use the share name directly (e.g. `/Public`, `/Backups`). Use `qnap shares` to see available paths.

## Environment Variables

All credentials and connection settings can be provided via environment variables, which take precedence over the config file and keychain. Useful for CI/CD and headless environments.

| Variable | Description |
|---|---|
| `QNAP_HOST` | NAS host (e.g. `192.168.1.50` or `https://nas.local`) |
| `QNAP_USERNAME` | Username |
| `QNAP_PASSWORD` | Password (bypasses OS keychain) |
| `QNAP_INSECURE` | Set to `1` to skip TLS certificate verification |
| `NO_COLOR` | Set to any value to disable colored output |

## Config File

Stores host, username, and the `insecure` flag. The password is **not** stored here.

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/qnap-cli/config.toml` |
| Linux | `~/.config/qnap-cli/config.toml` |
| Windows | `%APPDATA%\Roaming\qnap-cli\config.toml` |

## Agent Use

For AI agents, use `--json` on any command for structured output, and `qnap schema` to discover available commands and their output fields:

```sh
qnap schema          # prints full JSON schema
qnap info --json     # structured system info
qnap volumes --json  # volumes and disk array as JSON
```

The schema command output includes field types, enum values, error codes, and environment variable documentation.

## Compatibility

Tested against QTS 5.1.x and 5.2.x. The core authentication and file manager APIs have been stable since QTS 4.3. On headless Linux, use `QNAP_PASSWORD` to avoid requiring a desktop keyring service.

## Security

- Passwords are stored in the OS keychain (macOS Keychain, Linux Secret Service / kernel keyring, Windows Credential Manager)
- TLS verification is enabled by default; use `--insecure` or `QNAP_INSECURE=1` only for self-signed certificates on local networks
- The `--password` flag does not exist — passwords are never passed on the command line where they would appear in process listings

## License

MIT
