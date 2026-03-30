const SCHEMA: &str = r#"{
  "tool": "qnap",
  "description": "CLI for QNAP NAS management. Requires `qnap login` first.",
  "commands": {
    "login": {
      "description": "Save credentials and verify connectivity",
      "flags": [
        "--host <HOST>",
        "--username <USERNAME>",
        "--password <PASSWORD>",
        "--insecure (skip TLS verification)"
      ]
    },
    "info": {
      "description": "Show model, firmware, serial, hostname, uptime",
      "flags": ["--json"]
    },
    "status": {
      "description": "Show CPU usage, RAM usage, temperatures, uptime",
      "flags": ["--json"]
    },
    "volumes": {
      "description": "List storage volumes with size and RAID info",
      "flags": ["--json"]
    },
    "shares": {
      "description": "List shared folders",
      "flags": ["--json"]
    },
    "files ls <PATH>": {
      "description": "List files and directories at PATH on the NAS",
      "flags": ["--json"]
    },
    "files stat <PATH>": {
      "description": "Show metadata for a file or directory",
      "flags": ["--json"]
    }
  },
  "notes": [
    "Use --json for machine-readable output suitable for agents",
    "Default host is https://192.168.1.50",
    "Config stored at ~/.config/qnap-cli/config.toml"
  ]
}"#;

pub fn run() {
    println!("{}", SCHEMA);
}
