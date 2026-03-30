const SCHEMA: &str = concat!(
    r#"{
  "tool": "qnap",
  "version": ""#,
    env!("CARGO_PKG_VERSION"),
    r#"",
  "description": "CLI for QNAP NAS management. Run `qnap login` once to store credentials.",
  "supported_platforms": ["linux", "macos"],
  "global_flags": {
    "--host <HOST>": "Override the NAS host for a single command. HTTPS only.",
    "--username <USERNAME>": "Override the NAS username for a single command.",
    "--insecure": "Skip TLS certificate verification for a single command.",
    "--secure": "Re-enable TLS certificate verification for a single command.",
    "--password-stdin": "Read the password from stdin instead of the saved credentials file or interactive prompt."
  },
  "env": {
    "QNAP_HOST": "NAS host URL (overrides config file). HTTPS only.",
    "QNAP_USERNAME": "Username (overrides config file).",
    "QNAP_PASSWORD": "Password (overrides saved credentials file).",
    "QNAP_INSECURE": "Set to 1/true/yes to skip TLS verification, or 0/false/no to force verification.",
    "NO_COLOR": "Set to any value to disable colored output."
  },
  "commands": {
    "login": {
      "description": "Verify credentials against the NAS and save them. Host/username/TLS settings go to config.toml and the password goes to a separate credentials.toml file.",
      "notes": [
        "Prompts interactively for any missing fields.",
        "Uses QNAP_PASSWORD or the global --password-stdin flag for non-interactive auth.",
        "Rejects plaintext http:// hosts; HTTPS is required."
      ]
    },
    "info": {
      "description": "Show static system information.",
      "flags": { "--json": "Machine-readable JSON output." },
      "output": {
        "model": { "type": ["string", "null"], "example": "TS-453Be" },
        "hostname": { "type": ["string", "null"], "example": "NAS-123456" },
        "serial": { "type": ["string", "null"], "example": "Q123456789" },
        "firmware": { "type": ["string", "null"], "example": "5.1.0.2348" },
        "build": { "type": ["string", "null"], "example": "20230101" },
        "timezone": { "type": ["string", "null"], "example": "Europe/Amsterdam" },
        "uptime": {
          "type": ["object", "null"],
          "fields": {
            "display": { "type": "string", "example": "5d 12h 30m" },
            "days": { "type": "integer", "example": 5 },
            "hours": { "type": "integer", "example": 12 },
            "minutes": { "type": "integer", "example": 30 },
            "seconds": { "type": "integer", "example": 15 },
            "total_seconds": { "type": "integer", "example": 477015 }
          }
        }
      }
    },
    "status": {
      "description": "Show live system resource usage.",
      "flags": { "--json": "Machine-readable JSON output." },
      "output": {
        "cpu_usage_pct": { "type": ["number", "null"], "example": 12.5 },
        "mem_total_mb": { "type": ["integer", "null"], "example": 8192 },
        "mem_used_mb": { "type": ["integer", "null"], "example": 4096 },
        "mem_free_mb": { "type": ["integer", "null"], "example": 4096 },
        "temp_c": { "type": ["number", "null"], "example": 45.0 },
        "uptime": {
          "type": ["object", "null"],
          "fields": {
            "display": { "type": "string", "example": "5d 12h 30m" },
            "days": { "type": "integer", "example": 5 },
            "hours": { "type": "integer", "example": 12 },
            "minutes": { "type": "integer", "example": 30 },
            "seconds": { "type": "integer", "example": 15 },
            "total_seconds": { "type": "integer", "example": 477015 }
          }
        }
      }
    },
    "volumes": {
      "description": "List storage volumes and installed disks.",
      "flags": { "--json": "Machine-readable JSON output." },
      "output": {
        "volumes": {
          "type": "array",
          "items": {
            "label": { "type": "string", "example": "DataVol1" },
            "status": { "type": "string", "enum": ["ready", "not ready", "error", "degraded", "not active", "unknown"] },
            "status_code": { "type": ["integer", "null"], "example": -7 },
            "pool_id": { "type": ["integer", "null"], "example": 1 },
            "volume_type": { "type": "string", "example": "thick" }
          }
        },
        "disks": {
          "type": "array",
          "items": {
            "slot": { "type": "integer", "example": 1 },
            "model": { "type": "string", "example": "WD Blue 4TB SATAIII" },
            "kind": { "type": "string", "enum": ["HDD", "SSD"] },
            "temp_c": { "type": ["number", "null"], "example": 38.0 }
          }
        }
      }
    },
    "shares": {
      "description": "List NAS shared folders.",
      "flags": { "--json": "Machine-readable JSON output." },
      "output": {
        "type": "array",
        "items": {
          "name": { "type": "string", "example": "Public" },
          "path": { "type": "string", "example": "/Public", "note": "Use as PATH in `files ls`." },
          "items_count": { "type": ["integer", "null"], "example": 42 }
        }
      }
    },
    "files ls <PATH>": {
      "description": "List files and directories at PATH on the NAS.",
      "args": { "PATH": "Remote path, e.g. /Public or /Backups/photos." },
      "flags": {
        "--all": "Paginate until all results are returned.",
        "--json": "Machine-readable JSON output."
      },
      "output": {
        "type": "array",
        "items": {
          "entry_type": { "type": "string", "enum": ["file", "dir"] },
          "name": { "type": "string" },
          "size_bytes": { "type": ["integer", "null"], "example": 4096 },
          "modified": { "type": ["string", "null"], "example": "2026/02/16 18:24:37" },
          "modified_epoch": { "type": ["integer", "null"], "example": 1771262677 }
        }
      },
      "errors": {
        "path not found": "PATH does not exist on the NAS.",
        "permission denied": "Authenticated user lacks access to PATH."
      }
    },
    "files stat <PATH>": {
      "description": "Show normalized metadata for a file or directory plus the raw QNAP response object.",
      "args": { "PATH": "Remote path." },
      "flags": { "--json": "Machine-readable JSON output." },
      "output": {
        "path": { "type": "string", "example": "/Public/example.txt" },
        "name": { "type": ["string", "null"] },
        "entry_type": { "type": ["string", "null"], "enum": ["file", "dir"] },
        "size_bytes": { "type": ["integer", "null"] },
        "modified": { "type": ["string", "null"] },
        "modified_epoch": { "type": ["integer", "null"] },
        "owner": { "type": ["string", "null"] },
        "group": { "type": ["string", "null"] },
        "permissions": { "type": ["string", "null"] },
        "exists": { "type": ["boolean", "null"] },
        "is_link": { "type": ["boolean", "null"] },
        "raw": { "type": "object", "note": "Original QNAP metadata payload for fields not yet normalized." }
      }
    },
    "dump <DIR>": {
      "description": "Save raw API responses for compatibility debugging.",
      "args": { "DIR": "Output directory. Defaults to ./qnap-dump." }
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
