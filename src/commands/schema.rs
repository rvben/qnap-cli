use serde_json::{Value, json};

pub fn build_schema() -> Value {
    let mut schema = json!({
        "clispec": "0.3",
        "name": "qnap",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "CLI for QNAP NAS management.",
        "global_args": [
            {
                "name": "--output",
                "type": "string",
                "enum": ["auto", "text", "json"],
                "default": "auto",
                "description": "Output format. auto detects TTY and emits JSON when piped."
            },
            {
                "name": "--host",
                "type": "string",
                "required": false,
                "description": "Override the NAS host for this command (always HTTPS)."
            },
            {
                "name": "--username",
                "type": "string",
                "required": false,
                "description": "Override the NAS username for this command."
            },
            {
                "name": "--insecure",
                "type": "boolean",
                "required": false,
                "default": false,
                "description": "Skip TLS certificate verification for this command."
            },
            {
                "name": "--password-stdin",
                "type": "boolean",
                "required": false,
                "default": false,
                "description": "Read the password from stdin instead of the saved credentials file."
            }
        ],
        "commands": [
            {
                "name": "login",
                "description": "Verify credentials against the NAS and save them.",
                "mutating": true,
                "args": [],
                "output_fields": []
            },
            {
                "name": "info",
                "description": "Show static system information (model, firmware, hostname, uptime).",
                "mutating": false,
                "args": [],
                "output_fields": [
                    {"name": "model", "type": "string | null"},
                    {"name": "hostname", "type": "string | null"},
                    {"name": "serial", "type": "string | null"},
                    {"name": "firmware", "type": "string | null"},
                    {"name": "build", "type": "string | null"},
                    {"name": "timezone", "type": "string | null"},
                    {"name": "uptime", "type": "object | null", "description": "Fields: display, days, hours, minutes, seconds, total_seconds."}
                ]
            },
            {
                "name": "status",
                "description": "Show live system resource usage (CPU, RAM, temperature).",
                "mutating": false,
                "args": [],
                "output_fields": [
                    {"name": "cpu_usage_pct", "type": "number | null"},
                    {"name": "mem_total_mb", "type": "integer | null"},
                    {"name": "mem_used_mb", "type": "integer | null"},
                    {"name": "mem_free_mb", "type": "integer | null"},
                    {"name": "temp_c", "type": "number | null"},
                    {"name": "uptime", "type": "object | null", "description": "Fields: display, days, hours, minutes, seconds, total_seconds."}
                ]
            },
            {
                "name": "volumes",
                "description": "List storage volumes and installed disks.",
                "mutating": false,
                "args": [],
                "output_fields": [
                    {"name": "volumes", "type": "array", "description": "Array of volume objects with fields: label, status, status_code, pool_id, volume_type."},
                    {"name": "disks", "type": "array", "description": "Array of disk objects with fields: slot, model, kind, temp_c."}
                ]
            },
            {
                "name": "shares",
                "description": "List NAS shared folders.",
                "mutating": false,
                "args": [
                    {"name": "--limit", "type": "integer", "required": false, "default": 100, "description": "Maximum number of shares to return."},
                    {"name": "--offset", "type": "integer", "required": false, "default": 0, "description": "Number of shares to skip."},
                    {"name": "--fields", "type": "string", "required": false, "description": "Comma-separated list of fields to include in JSON output."}
                ],
                "output_fields": [
                    {"name": "items", "type": "array", "description": "Array of share objects: name, path, items_count."},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "network",
                "description": "Show network adapter information.",
                "mutating": false,
                "args": [],
                "output_fields": [
                    {"name": "adapters", "type": "array", "description": "Array of NIC objects: name, display_name, ip, mask, mac, status, speed_mbps, mode, rx_packets, tx_packets, err_packets."},
                    {"name": "dns", "type": "array", "description": "List of DNS server addresses."}
                ]
            },
            {
                "name": "config",
                "description": "Show current saved configuration.",
                "mutating": false,
                "args": [],
                "output_fields": [
                    {"name": "host", "type": "string | null"},
                    {"name": "username", "type": "string | null"},
                    {"name": "tls_verify", "type": "boolean"},
                    {"name": "config_file", "type": "string"},
                    {"name": "credentials_file", "type": "string"}
                ]
            },
            {
                "name": "dump",
                "description": "Save raw API responses for debugging and compatibility reporting.",
                "mutating": true,
                "args": [
                    {"name": "dir", "type": "string", "required": false, "default": "./qnap-dump", "description": "Directory to write response files into."}
                ],
                "output_fields": []
            },
            {
                "name": "schema",
                "description": "Print the clispec v0.3 command schema for agent use.",
                "mutating": false,
                "args": [],
                "output_fields": []
            },
            {
                "name": "completions",
                "description": "Generate shell completion script.",
                "mutating": false,
                "args": [
                    {"name": "shell", "type": "string", "required": true, "enum": ["bash", "fish", "zsh", "powershell", "elvish"], "description": "Shell to generate completions for."}
                ],
                "output_fields": []
            },
            {
                "name": "files ls",
                "description": "List files and directories at PATH on the NAS.",
                "mutating": false,
                "args": [
                    {"name": "path", "type": "string", "required": true, "description": "Remote path, e.g. /Public or /Backups/photos."},
                    {"name": "--all", "type": "boolean", "required": false, "default": false, "description": "Paginate until all results are returned."},
                    {"name": "--recursive", "type": "boolean", "required": false, "default": false, "description": "Recursively list all files in subdirectories."},
                    {"name": "--limit", "type": "integer", "required": false, "default": 100, "description": "Maximum number of items to return."},
                    {"name": "--offset", "type": "integer", "required": false, "default": 0, "description": "Number of items to skip."},
                    {"name": "--fields", "type": "string", "required": false, "description": "Comma-separated list of fields to include in JSON output."}
                ],
                "output_fields": [
                    {"name": "items", "type": "array", "description": "Array of file objects: entry_type, name, size_bytes, modified, modified_epoch."},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "files stat",
                "description": "Show normalized metadata for a file or directory.",
                "mutating": false,
                "args": [
                    {"name": "path", "type": "string", "required": true, "description": "Remote path."}
                ],
                "output_fields": [
                    {"name": "path", "type": "string"},
                    {"name": "name", "type": "string | null"},
                    {"name": "entry_type", "type": "string | null"},
                    {"name": "size_bytes", "type": "integer | null"},
                    {"name": "modified", "type": "string | null"},
                    {"name": "modified_epoch", "type": "integer | null"},
                    {"name": "owner", "type": "string | null"},
                    {"name": "group", "type": "string | null"},
                    {"name": "permissions", "type": "string | null"},
                    {"name": "exists", "type": "boolean | null"},
                    {"name": "is_link", "type": "boolean | null"},
                    {"name": "raw", "type": "object"}
                ]
            },
            {
                "name": "files mkdir",
                "description": "Create a directory on the NAS.",
                "mutating": true,
                "args": [
                    {"name": "path", "type": "string", "required": true, "description": "Remote path to create, e.g. /Public/newdir."},
                    {"name": "--yes", "type": "boolean", "required": false, "default": false, "description": "Confirm the operation without prompting (required when not in a TTY)."}
                ],
                "output_fields": []
            },
            {
                "name": "files rm",
                "description": "Delete one or more files or directories from the NAS.",
                "mutating": true,
                "args": [
                    {"name": "paths", "type": "string[]", "required": true, "description": "Remote paths to delete."},
                    {"name": "--yes", "type": "boolean", "required": false, "default": false, "description": "Confirm the operation without prompting (required when not in a TTY)."}
                ],
                "output_fields": []
            },
            {
                "name": "files mv",
                "description": "Move or rename a file or directory on the NAS.",
                "mutating": true,
                "args": [
                    {"name": "src", "type": "string", "required": true, "description": "Source remote path."},
                    {"name": "dst", "type": "string", "required": true, "description": "Destination remote path."},
                    {"name": "--yes", "type": "boolean", "required": false, "default": false, "description": "Confirm the operation without prompting (required when not in a TTY)."}
                ],
                "output_fields": []
            },
            {
                "name": "files cp",
                "description": "Copy a file or directory on the NAS.",
                "mutating": true,
                "args": [
                    {"name": "src", "type": "string", "required": true, "description": "Source remote path."},
                    {"name": "dst", "type": "string", "required": true, "description": "Destination remote path."},
                    {"name": "--overwrite", "type": "boolean", "required": false, "default": false, "description": "Overwrite if destination exists."},
                    {"name": "--yes", "type": "boolean", "required": false, "default": false, "description": "Confirm the operation without prompting (required when not in a TTY)."}
                ],
                "output_fields": []
            },
            {
                "name": "files upload",
                "description": "Upload a local file or directory to the NAS.",
                "mutating": true,
                "args": [
                    {"name": "local", "type": "string", "required": true, "description": "Local file or directory to upload."},
                    {"name": "remote_dir", "type": "string", "required": true, "description": "Remote directory to upload into, e.g. /Public."},
                    {"name": "--overwrite", "type": "boolean", "required": false, "default": false, "description": "Overwrite if file already exists."},
                    {"name": "--recursive", "type": "boolean", "required": false, "default": false, "description": "Recursively upload a directory and its contents."},
                    {"name": "--yes", "type": "boolean", "required": false, "default": false, "description": "Confirm the operation without prompting (required when not in a TTY)."}
                ],
                "output_fields": []
            },
            {
                "name": "files download",
                "description": "Download a file or directory from the NAS.",
                "mutating": true,
                "args": [
                    {"name": "remote", "type": "string", "required": true, "description": "Remote file or directory path, e.g. /Public/photos."},
                    {"name": "local", "type": "string", "required": false, "description": "Local path to save to (defaults to name in current directory)."},
                    {"name": "--recursive", "type": "boolean", "required": false, "default": false, "description": "Recursively download a directory and its contents."}
                ],
                "output_fields": []
            },
            {
                "name": "files find",
                "description": "Search for files matching a glob pattern on the NAS.",
                "mutating": false,
                "args": [
                    {"name": "path", "type": "string", "required": true, "description": "Remote path to search under, e.g. /Public."},
                    {"name": "pattern", "type": "string", "required": true, "description": "Glob pattern to match filenames, e.g. *.txt or backup*."},
                    {"name": "--limit", "type": "integer", "required": false, "default": 100, "description": "Maximum number of matches to return."},
                    {"name": "--offset", "type": "integer", "required": false, "default": 0, "description": "Number of matches to skip."},
                    {"name": "--fields", "type": "string", "required": false, "description": "Comma-separated fields to include in JSON output."}
                ],
                "output_fields": [
                    {"name": "path", "type": "string"},
                    {"name": "entry_type", "type": "string"},
                    {"name": "size_bytes", "type": "integer | null"},
                    {"name": "modified", "type": "string | null"}
                ]
            }
        ],
        "errors": [
            {"kind": "auth", "exit_code": 4, "retryable": false, "description": "Authentication failed or credentials are invalid."},
            {"kind": "not_found", "exit_code": 2, "retryable": false, "description": "The requested resource does not exist on the NAS."},
            {"kind": "permission_denied", "exit_code": 3, "retryable": false, "description": "The authenticated user lacks access to the resource."},
            {"kind": "network_error", "exit_code": 5, "retryable": true, "description": "Failed to reach the NAS. Check host and network connectivity."},
            {"kind": "confirmation_required", "exit_code": 2, "retryable": false, "description": "A destructive operation requires --yes confirmation when not in a TTY."},
            {"kind": "general", "exit_code": 1, "retryable": false, "description": "Unexpected error."}
        ]
    });
    enrich_v0_3(&mut schema);
    schema
}

fn enrich_v0_3(schema: &mut Value) {
    schema["output"] = json!({"tty": "text", "piped": "json"});
    let Some(commands) = schema["commands"].as_array_mut() else {
        return;
    };
    for command in commands {
        let Some(object) = command.as_object_mut() else {
            continue;
        };
        let name = object["name"].as_str().unwrap_or_default().to_string();
        let mutating = object["mutating"].as_bool().unwrap_or(false);
        object.insert(
            "effects".into(),
            json!(if !mutating {
                "read_only"
            } else if matches!(name.as_str(), "login" | "files mkdir" | "files rm") {
                "idempotent"
            } else {
                "non_idempotent"
            }),
        );

        if name == "completions" {
            object.remove("output_fields");
            object.insert("output_kind".into(), json!("opaque"));
            object.insert("media_type".into(), json!("text/plain"));
            continue;
        }

        let unbounded = matches!(name.as_str(), "shares" | "files ls" | "files find");
        object.insert(
            "cardinality".into(),
            json!(if unbounded { "unbounded" } else { "bounded" }),
        );
        if unbounded {
            object.insert(
                "pagination".into(),
                json!({"style":"offset","limit_arg":"--limit","offset_arg":"--offset"}),
            );
            object.insert("fields_arg".into(), json!("--fields"));
        }
        if matches!(
            name.as_str(),
            "files mkdir" | "files rm" | "files mv" | "files cp" | "files upload"
        ) {
            object.insert("confirmation_bypass_arg".into(), json!("--yes"));
        }
        if name == "config" {
            object.insert("example".into(), json!({"args":["config"]}));
        }
        if name == "schema" {
            object.remove("output_fields");
            object.insert("cardinality".into(), json!("single"));
            object.insert(
                "stdout_schema".into(),
                json!({"$ref":"https://clispec.dev/schema/v0.3.json"}),
            );
        }

        if let Some(fields) = object
            .get_mut("output_fields")
            .and_then(Value::as_array_mut)
        {
            for field in fields {
                let Some(field) = field.as_object_mut() else {
                    continue;
                };
                if let Some(base) = field
                    .get("type")
                    .and_then(Value::as_str)
                    .and_then(|kind| kind.strip_suffix(" | null"))
                    .map(str::to_owned)
                {
                    field.insert("type".into(), json!(base));
                    field.insert("nullable".into(), json!(true));
                }
                if field.get("type").and_then(Value::as_str) == Some("array")
                    && !field.contains_key("items")
                {
                    let item_type = if field.get("name").and_then(Value::as_str) == Some("dns") {
                        "string"
                    } else {
                        "object"
                    };
                    field.insert("items".into(), json!({"type": item_type}));
                }
            }
        }
        if !object.contains_key("output_fields") && !object.contains_key("stdout_schema") {
            object.insert("stdout_schema".into(), json!({}));
        }
    }
}

pub fn run() {
    let schema = build_schema();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLISPEC_V03_SCHEMA: &str = include_str!("../../tests/fixtures/clispec-v0.3.json");

    #[test]
    fn schema_validates_against_clispec_v03() {
        let meta_schema: serde_json::Value = serde_json::from_str(CLISPEC_V03_SCHEMA)
            .expect("clispec v0.3 fixture is not valid JSON");
        let validator = jsonschema::validator_for(&meta_schema)
            .expect("clispec v0.3 fixture is not a valid JSON Schema");

        let output = build_schema();
        let errors: Vec<String> = validator
            .iter_errors(&output)
            .map(|e| e.to_string())
            .collect();
        if !errors.is_empty() {
            panic!(
                "schema output failed clispec v0.3 validation:\n{}",
                errors.join("\n")
            );
        }
    }

    #[test]
    fn schema_has_required_top_level_fields() {
        let schema = build_schema();
        assert_eq!(schema["clispec"], "0.3", "clispec field must be '0.3'");
        assert_eq!(schema["name"], "qnap", "name field must be 'qnap'");
        assert!(
            schema["version"].as_str().is_some_and(|v| !v.is_empty()),
            "version must be a non-empty string"
        );
        assert!(
            schema["commands"].as_array().is_some_and(|a| !a.is_empty()),
            "commands must be a non-empty array"
        );
        assert!(
            schema["errors"].as_array().is_some_and(|a| !a.is_empty()),
            "errors must be a non-empty array"
        );
        assert!(
            schema["global_args"].as_array().is_some(),
            "global_args must be an array"
        );
    }

    #[test]
    fn all_commands_have_mutating_field() {
        let schema = build_schema();
        let commands = schema["commands"].as_array().unwrap();
        for cmd in commands {
            let name = cmd["name"].as_str().unwrap_or("unknown");
            assert!(
                cmd.get("mutating").and_then(|v| v.as_bool()).is_some(),
                "command '{}' is missing the mutating field",
                name
            );
        }
    }

    #[test]
    fn all_error_kinds_have_exit_codes() {
        let schema = build_schema();
        let errors = schema["errors"].as_array().unwrap();
        for error in errors {
            let kind = error["kind"].as_str().unwrap_or("unknown");
            assert!(
                error.get("exit_code").and_then(|v| v.as_u64()).is_some(),
                "error kind '{}' is missing exit_code",
                kind
            );
        }
    }

    #[test]
    fn conflict_error_kind_is_not_declared() {
        // The code maps "destination already exists" to the general error kind (exit 1).
        // There is no reachable path that emits a structured conflict kind, so the
        // schema must not advertise one.
        let schema = build_schema();
        let errors = schema["errors"].as_array().unwrap();
        let has_conflict = errors
            .iter()
            .any(|e| e["kind"].as_str() == Some("conflict"));
        assert!(
            !has_conflict,
            "errors array must not include a 'conflict' kind — it is unreachable in the current code"
        );
    }

    #[test]
    fn global_args_include_output_flag() {
        let schema = build_schema();
        let args = schema["global_args"].as_array().unwrap();
        let has_output = args.iter().any(|a| a["name"].as_str() == Some("--output"));
        assert!(has_output, "global_args must include '--output'");
    }

    #[test]
    fn list_commands_have_pagination_args() {
        let schema = build_schema();
        let commands = schema["commands"].as_array().unwrap();
        for cmd in commands {
            let name = cmd["name"].as_str().unwrap_or("");
            if name == "shares" || name == "files ls" {
                let empty = vec![];
                let args = cmd["args"].as_array().unwrap_or(&empty);
                let has_limit = args.iter().any(|a| a["name"] == "--limit");
                let has_offset = args.iter().any(|a| a["name"] == "--offset");
                assert!(has_limit, "command '{}' must have a --limit arg", name);
                assert!(has_offset, "command '{}' must have an --offset arg", name);
            }
        }
    }
}
