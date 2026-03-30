const SCHEMA: &str = concat!(
    r#"{
  "tool": "qnap",
  "version": ""#,
    env!("CARGO_PKG_VERSION"),
    r#"",
  "description": "CLI for QNAP NAS management. Run `qnap login` once to store credentials.",
  "env": {
    "QNAP_HOST": "NAS host URL (overrides config file)",
    "QNAP_USERNAME": "Username (overrides config file)",
    "QNAP_PASSWORD": "Password (overrides keychain)",
    "QNAP_INSECURE": "Set to '1' to skip TLS verification",
    "NO_COLOR": "Set to any value to disable colored output"
  },
  "commands": {
    "login": {
      "description": "Verify credentials against the NAS and save them. Password is stored in the OS keychain (never in the config file).",
      "flags": {
        "--host <HOST>": "NAS host (e.g. 192.168.1.50 or https://nas.local)",
        "--username <USERNAME>": "NAS username",
        "--insecure": "Skip TLS certificate verification (for self-signed certs)"
      },
      "notes": ["Prompts interactively for any missing fields", "Prints config file location on success"]
    },
    "info": {
      "description": "Show static system information.",
      "flags": { "--json": "Machine-readable JSON output" },
      "output": {
        "model": { "type": "string", "example": "TS-453Be" },
        "hostname": { "type": "string", "example": "NAS-123456" },
        "serial": { "type": "string", "example": "Q123456789" },
        "firmware": { "type": "string", "example": "5.1.0.2348" },
        "build": { "type": "string", "example": "20230101" },
        "timezone": { "type": "string", "example": "Europe/Amsterdam" },
        "uptime": { "type": "string", "example": "5d 12h 30m" }
      }
    },
    "status": {
      "description": "Show live system resource usage.",
      "flags": { "--json": "Machine-readable JSON output" },
      "output": {
        "cpu_usage": { "type": "string", "example": "12%", "note": "Current CPU utilisation" },
        "mem_total_mb": { "type": "string", "example": "8192", "unit": "MB" },
        "mem_used_mb": { "type": "string", "example": "4096", "unit": "MB" },
        "mem_free_mb": { "type": "string", "example": "4096", "unit": "MB" },
        "temp": { "type": "string", "example": "45°C", "note": "System temperature; colored green/yellow/red in terminal" },
        "uptime": { "type": "string", "example": "5d 12h 30m" }
      }
    },
    "volumes": {
      "description": "List storage volumes and installed disks.",
      "flags": { "--json": "Machine-readable JSON output" },
      "output": {
        "volumes": {
          "type": "array",
          "items": {
            "label": { "type": "string", "example": "DataVol1" },
            "status": {
              "type": "string",
              "enum": ["ready", "not ready", "error", "degraded", "not active", "unknown"],
              "note": "Colored in terminal: green=ready, red=error/degraded"
            },
            "pool": { "type": "string", "example": "1" },
            "type": { "type": "string", "example": "thin" }
          }
        },
        "disks": {
          "type": "array",
          "items": {
            "slot": { "type": "string", "example": "1" },
            "model": { "type": "string", "example": "WD Blue 4TB SATAIII" },
            "kind": { "type": "string", "enum": ["HDD", "SSD"] },
            "temp": { "type": "string", "example": "38°C", "note": "'-' if unavailable" }
          }
        }
      }
    },
    "shares": {
      "description": "List NAS shared folders.",
      "flags": { "--json": "Machine-readable JSON output" },
      "output": {
        "type": "array",
        "items": {
          "name": { "type": "string", "example": "Public" },
          "path": { "type": "string", "example": "/Public", "note": "Use as PATH in `files ls`" },
          "items": { "type": "string", "example": "42", "note": "Top-level item count" }
        }
      }
    },
    "files ls <PATH>": {
      "description": "List files and directories at PATH on the NAS.",
      "args": { "PATH": "Remote path, e.g. /Public or /Backups/photos" },
      "flags": { "--json": "Machine-readable JSON output" },
      "output": {
        "type": "array",
        "items": {
          "type": { "type": "string", "enum": ["file", "dir"] },
          "name": { "type": "string" },
          "size": { "type": "string", "note": "File size in bytes as string; '-' for directories" },
          "modified": { "type": "string", "note": "Modification timestamp" }
        }
      },
      "errors": {
        "path not found": "PATH does not exist on the NAS",
        "permission denied": "Authenticated user lacks access to PATH"
      }
    },
    "files stat <PATH>": {
      "description": "Show raw metadata for a file or directory.",
      "args": { "PATH": "Remote path" },
      "flags": { "--json": "Machine-readable JSON output" }
    },
    "schema": {
      "description": "Print this schema document. Designed for agent introspection.",
      "flags": {}
    }
  }
}"#
);

pub fn run() {
    println!("{}", SCHEMA);
}
